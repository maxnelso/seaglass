"""Minimal masked-PPO training loop and bot evaluation CLI."""

from __future__ import annotations

import argparse
import time
import numpy as np
import seaglass
import torch
import torch.nn as nn

from .actions import ACTION_SPACE_SIZE, END_TURN_INDEX, action_from_index, action_mask
from .bots import finish_turn
from .env import BattlegroundsEnv, compute_board_power
from .features import NUM_SLOTS, PLAYER_DIM, SLOT_NUM_DIM, encode_observation
from .model import BattlegroundsMLP


def _fmt_unit(u) -> str:
    if u.is_spell:
        curr = "HP" if u.costs_health else "g"
        return f'[Spell] "{u.name}" (T{u.tavern_tier} {u.spell_cost}{curr})'
    star = "★ " if u.is_golden else ""
    kws = [
        kw
        for kw, has in [
            ("Taunt", u.taunt),
            ("DS", u.divine_shield),
            ("WF", u.windfury),
            ("Reborn", u.reborn),
            ("Venom", u.venomous),
            ("Mag", u.magnetic),
        ]
        if has
    ]
    kw_str = f" [{', '.join(kws)}]" if kws else ""
    return f'"{star}{u.name}" {u.attack}/{u.health} (T{u.tavern_tier} {u.tribe}){kw_str}'


def _fmt_zone(units) -> str:
    if not units:
        return "(empty)"
    return ", ".join(f"[{i}] {_fmt_unit(u)}" for i, u in enumerate(units))


def _fmt_action(act, obs) -> str:
    kind = act.kind
    a0, a1 = act.args
    if kind == "buy" and a0 is not None and a0 < len(obs.shop):
        return f"Buy({a0}) -> {_fmt_unit(obs.shop[a0])}"
    if kind == "play" and a0 is not None and a0 < len(obs.hand):
        return f"Play(hand={a0}, pos={a1}) -> {_fmt_unit(obs.hand[a0])}"
    if kind == "play_on_shop" and a0 is not None and a0 < len(obs.hand):
        return f"PlayOnShop(hand={a0}, shop={a1}) -> {_fmt_unit(obs.hand[a0])}"
    if kind == "sell" and a0 is not None and a0 < len(obs.board):
        return f"Sell({a0}) -> {_fmt_unit(obs.board[a0])}"
    if kind == "activate" and a0 is not None and a0 < len(obs.board):
        return f"Activate(pos={a0}, target={a1}) -> {_fmt_unit(obs.board[a0])}"
    if kind == "choose_discover" and obs.discover_pending and a0 < len(obs.discover_pending):
        return f"ChooseDiscover({a0}) -> {_fmt_unit(obs.discover_pending[a0])}"
    return repr(act)


def evaluate_policy(
    model: BattlegroundsMLP,
    opponent: str = "random",
    episodes: int = 20,
    seed: int = 10_000,
    deterministic: bool = False,
) -> dict[str, float]:
    env = BattlegroundsEnv(opponent=opponent, seed=seed, model=model)
    wins = 0
    draws = 0
    total_turns = 0

    for ep in range(episodes):
        obs, mask = env.reset(seed=seed + ep * 17)
        done = False
        info = {}
        while not done:
            action, _, _ = model.act(obs, mask, deterministic=deterministic)
            obs, _, done, mask, info = env.step(action)
        if info.get("winner") == "agent":
            wins += 1
        elif info.get("winner") == "draw":
            draws += 1
        total_turns += info.get("turns", 0)

    return {
        "win_rate": wins / episodes,
        "draw_rate": draws / episodes,
        "avg_turns": total_turns / episodes,
    }


def collect_selfplay_rollouts(
    model: BattlegroundsMLP,
    games_per_iter: int,
    seed_base: int,
    game_counter_start: int,
    max_turns: int = 20,
    max_steps_per_turn: int = 15,
    gamma: float = 0.99,
    gae_lambda: float = 0.95,
    record_last_game: bool = False,
) -> dict[str, torch.Tensor | float | str]:
    """Lockstep batched self-play collector (trains on both Player A and Player B trajectories)."""
    N = max(1, games_per_iter)
    P = 2 * N
    games = [
        seaglass.TavernGame(
            seed=seed_base + i,
            catalog="full",
            deity="cthun" if i % 2 == 0 else "yshaarj",
            trinkets=True,
            dark_gifts=True,
        )
        for i in range(P)
    ]
    for g in games:
        g.start_turn()

    traj: list[list[list]] = [[] for _ in range(P)]
    alive_games = [True] * N
    a_wins = 0
    trace_lines: list[str] = []
    last_gi = N - 1
    last_pa = 2 * last_gi
    last_pb = 2 * last_gi + 1

    if record_last_game:
        trace_lines.append("=" * 92)
        trace_lines.append(
            f" SEAGLASS RL — LAST SELF-PLAY GAME TRACE (Game #{game_counter_start + N}: Player A vs Player B)"
        )
        trace_lines.append("=" * 92)

    p_buf = np.empty((P, PLAYER_DIM), dtype=np.float32)
    c_buf = np.empty((P, NUM_SLOTS), dtype=np.int64)
    n_buf = np.empty((P, NUM_SLOTS, SLOT_NUM_DIM), dtype=np.float32)
    m_buf = np.empty((P, ACTION_SPACE_SIZE), dtype=np.bool_)

    for _turn_idx in range(max_turns):
        if not any(alive_games):
            break

        traced_this_turn = record_last_game and alive_games[last_gi]
        if traced_this_turn:
            oa_start = games[last_pa].observe()
            ob_start = games[last_pb].observe()
            trace_lines.append(
                f"\n{'=' * 30} TURN {oa_start.turn} {'=' * 30}"
            )
            p_logs: dict[int, list[str]] = {
                last_pa: [
                    f"  [Player A Start | HP: {oa_start.health}+{oa_start.armor} | "
                    f"Tier {oa_start.tavern_tier} (upg={oa_start.upgrade_cost}g) | Gold: {oa_start.gold}/{oa_start.max_gold}]",
                    f"    Board : {_fmt_zone(oa_start.board)}",
                    f"    Hand  : {_fmt_zone(oa_start.hand)}",
                    f"    Shop  : {_fmt_zone(oa_start.shop)}",
                ],
                last_pb: [
                    f"  [Player B Start | HP: {ob_start.health}+{ob_start.armor} | "
                    f"Tier {ob_start.tavern_tier} (upg={ob_start.upgrade_cost}g) | Gold: {ob_start.gold}/{ob_start.max_gold}]",
                    f"    Board : {_fmt_zone(ob_start.board)}",
                    f"    Hand  : {_fmt_zone(ob_start.hand)}",
                    f"    Shop  : {_fmt_zone(ob_start.shop)}",
                ],
            }
            p_step_num = {last_pa: 0, last_pb: 0}

        active = [p for p in range(P) if alive_games[p // 2]]
        steps = 0
        while active and steps < max_steps_per_turn:
            B = len(active)
            p_np = p_buf[:B]
            c_np = c_buf[:B]
            n_np = n_buf[:B]
            m_np = m_buf[:B]
            raw_obs_list = []
            pre_powers = []

            for bi, p in enumerate(active):
                raw = games[p].observe()
                raw_obs_list.append(raw)
                pv, ci, nf = encode_observation(raw)
                p_np[bi] = pv
                c_np[bi] = ci
                n_np[bi] = nf
                m_np[bi] = action_mask(games[p], prune_dominated=True, obs=raw)
                pre_powers.append(compute_board_power(raw))

            acts, act_lps, vals = model.act_batch(p_np, c_np, n_np, m_np, deterministic=False)
            next_active = []

            for bi, p in enumerate(active):
                a = int(acts[bi])
                eng_act = action_from_index(a)
                if traced_this_turn and p in p_logs:
                    desc = _fmt_action(eng_act, raw_obs_list[bi])

                games[p].step(eng_act)
                ended = a == END_TURN_INDEX
                if not ended and steps + 1 >= max_steps_per_turn:
                    finish_turn(games[p])
                    ended = True

                post_raw = games[p].observe()
                rew = compute_board_power(post_raw) - pre_powers[bi]
                traj[p].append(
                    [
                        p_np[bi].copy(),
                        c_np[bi].copy(),
                        n_np[bi].copy(),
                        m_np[bi].copy(),
                        a,
                        float(act_lps[bi]),
                        rew,
                        float(vals[bi]),
                    ]
                )

                if traced_this_turn and p in p_logs:
                    p_step_num[p] += 1
                    p_logs[p].append(
                        f"    [Step {p_step_num[p]:02d}] {desc}  "
                        f"(rew={rew:+.3f}, V={float(vals[bi]):+.2f}, gold={post_raw.gold})"
                    )

                if not ended:
                    next_active.append(p)

            active = next_active
            steps += 1

        # Resolve combat for each alive game
        for gi in range(N):
            if not alive_games[gi]:
                continue
            pa, pb = 2 * gi, 2 * gi + 1
            if traced_this_turn and gi == last_gi:
                pre_ca = games[pa].observe()
                pre_cb = games[pb].observe()

            battle = games[pa].resolve_combat_with(games[pb])
            combat_delta_a = 0.0
            if battle.outcome == "a_win":
                combat_delta_a = 0.2 + battle.hero_damage / 30.0
            elif battle.outcome == "b_win":
                combat_delta_a = -(0.2 + battle.hero_damage / 30.0)

            traj[pa][-1][6] += combat_delta_a
            traj[pb][-1][6] -= combat_delta_a

            oa = games[pa].observe()
            ob = games[pb].observe()
            game_over = oa.health <= 0 or ob.health <= 0 or oa.turn >= max_turns
            term_a = 0.0
            winner_str = "DRAW"
            if game_over:
                alive_games[gi] = False
                if oa.health > ob.health:
                    term_a = 2.0 if ob.health <= 0 else 1.0
                    a_wins += 1
                    winner_str = "PLAYER A"
                elif ob.health > oa.health:
                    term_a = -(2.0 if oa.health <= 0 else 1.0)
                    winner_str = "PLAYER B"
                traj[pa][-1][6] += term_a
                traj[pb][-1][6] -= term_a
            else:
                games[pa].start_turn()
                games[pb].start_turn()

            if traced_this_turn and gi == last_gi:
                trace_lines.extend(p_logs[last_pa])
                trace_lines.append("")
                trace_lines.extend(p_logs[last_pb])
                trace_lines.append(f"\n  [Combat Turn {pre_ca.turn}]")
                trace_lines.append(f"    Player A Board (T{pre_ca.tavern_tier}): {_fmt_zone(pre_ca.board)}")
                trace_lines.append(f"    Player B Board (T{pre_cb.tavern_tier}): {_fmt_zone(pre_cb.board)}")
                trace_lines.append(
                    f"    Result: {battle.outcome} (hero_damage={battle.hero_damage}) | "
                    f"Player A HP: {oa.health}+{oa.armor} | Player B HP: {ob.health}+{ob.armor}"
                )
                if battle.survivors_a:
                    trace_lines.append(f"    Player A Survivors: {_fmt_zone(battle.survivors_a)}")
                if battle.survivors_b:
                    trace_lines.append(f"    Player B Survivors: {_fmt_zone(battle.survivors_b)}")
                if game_over:
                    rew_a = sum(r[6] for r in traj[last_pa])
                    rew_b = sum(r[6] for r in traj[last_pb])
                    trace_lines.append("\n" + "=" * 92)
                    trace_lines.append(
                        f" GAME OVER ON TURN {oa.turn}: Winner = {winner_str} | "
                        f"Player A HP: {oa.health} (Rew: {rew_a:+.2f}) vs "
                        f"Player B HP: {ob.health} (Rew: {rew_b:+.2f})"
                    )
                    trace_lines.append("=" * 92 + "\n")

    p_vecs, c_ids, n_feats, masks, actions, old_lps, returns, advs = [], [], [], [], [], [], [], []
    ep_rewards = []
    for t_list in traj:
        T = len(t_list)
        ep_rewards.append(sum(r[6] for r in t_list))
        adv = np.zeros(T, dtype=np.float32)
        last_gae = 0.0
        for t in reversed(range(T)):
            nv = 0.0 if t == T - 1 else t_list[t + 1][7]
            delta = t_list[t][6] + gamma * nv - t_list[t][7]
            last_gae = delta + gamma * gae_lambda * last_gae
            adv[t] = last_gae
        for t, row in enumerate(t_list):
            p_vecs.append(row[0])
            c_ids.append(row[1])
            n_feats.append(row[2])
            masks.append(row[3])
            actions.append(row[4])
            old_lps.append(row[5])
            returns.append(adv[t] + row[7])
            advs.append(adv[t])

    adv_t = torch.as_tensor(np.asarray(advs), dtype=torch.float32)
    if adv_t.numel() > 1:
        adv_t = (adv_t - adv_t.mean()) / (adv_t.std() + 1e-8)

    return {
        "player_vec": torch.from_numpy(np.asarray(p_vecs)),
        "slot_card_ids": torch.from_numpy(np.asarray(c_ids)),
        "slot_num_feats": torch.from_numpy(np.asarray(n_feats)),
        "action_mask": torch.from_numpy(np.asarray(masks)),
        "actions": torch.as_tensor(np.asarray(actions), dtype=torch.long),
        "old_log_probs": torch.as_tensor(np.asarray(old_lps), dtype=torch.float32),
        "returns": torch.as_tensor(np.asarray(returns), dtype=torch.float32),
        "advantages": adv_t,
        "win_rate": a_wins / N,
        "mean_reward": float(np.mean(ep_rewards)),
        "num_steps": len(actions),
        "last_trace": "\n".join(trace_lines),
    }


def collect_rollouts(
    env: BattlegroundsEnv,
    model: BattlegroundsMLP,
    episodes: int,
    gamma: float = 0.99,
    gae_lambda: float = 0.95,
    record_last_episode: bool = False,
) -> dict[str, torch.Tensor | float | str]:
    p_vecs, c_ids, n_feats, masks, actions, old_log_probs, returns, advantages = (
        [],
        [],
        [],
        [],
        [],
        [],
        [],
        [],
    )
    wins = 0
    ep_rewards = []
    last_trace_lines: list[str] = []

    for ep_idx in range(episodes):
        trace_this = record_last_episode and (ep_idx == episodes - 1)
        obs, mask = env.reset()
        done = False
        ep_p, ep_c, ep_n, ep_m, ep_a, ep_lp, ep_r, ep_v = [], [], [], [], [], [], [], []
        info = {}
        cur_turn = -1
        step_in_turn = 0

        if trace_this:
            last_trace_lines.append("=" * 88)
            last_trace_lines.append(
                f" SEAGLASS RL — LAST TRAINING GAME TRACE (Episode #{env.episode_count}, Opponent: {env.opponent_kind})"
            )
            last_trace_lines.append("=" * 88)

        while not done:
            raw_obs = env.player.observe() if trace_this else None
            if trace_this and raw_obs.turn != cur_turn:
                cur_turn = raw_obs.turn
                step_in_turn = 0
                last_trace_lines.append(
                    f"\n--- TURN {raw_obs.turn} START | Agent HP: {raw_obs.health}+{raw_obs.armor} | "
                    f"Tier {raw_obs.tavern_tier} (upg={raw_obs.upgrade_cost}g) | Gold: {raw_obs.gold}/{raw_obs.max_gold} ---"
                )
                last_trace_lines.append(f"  Board : {_fmt_zone(raw_obs.board)}")
                last_trace_lines.append(f"  Hand  : {_fmt_zone(raw_obs.hand)}")
                last_trace_lines.append(f"  Shop  : {_fmt_zone(raw_obs.shop)}")

            act, lp, val = model.act(obs, mask, deterministic=False)
            if trace_this:
                step_in_turn += 1
                eng_act = action_from_index(act)
                desc = _fmt_action(eng_act, raw_obs)

            next_obs, rew, done, next_mask, info = env.step(act)

            if trace_this:
                post_raw = env.player.observe()
                last_trace_lines.append(
                    f"  [Step {step_in_turn:02d}] {desc}  (rew={rew:+.3f}, V={val:+.2f}, gold={post_raw.gold})"
                )
                if "combat" in info:
                    c = info["combat"]
                    b = c["battle"]
                    pa = c["post_agent"]
                    po = c["post_opp"]
                    last_trace_lines.append(f"  [Combat Turn {c['turn']}]")
                    last_trace_lines.append(f"    Agent Final Board : {_fmt_zone(c['agent_board'])}")
                    last_trace_lines.append(
                        f"    Opp Final Board (T{c['opp_tier']}): {_fmt_zone(c['opp_board'])}"
                    )
                    last_trace_lines.append(
                        f"    Result: {b.outcome} (hero_damage={b.hero_damage}) | "
                        f"Agent HP: {pa.health}+{pa.armor} | Opp HP: {po.health}+{po.armor}"
                    )
                    if b.survivors_a:
                        last_trace_lines.append(f"    Agent Survivors   : {_fmt_zone(b.survivors_a)}")
                    if b.survivors_b:
                        last_trace_lines.append(f"    Opp Survivors     : {_fmt_zone(b.survivors_b)}")

            ep_p.append(obs[0])
            ep_c.append(obs[1])
            ep_n.append(obs[2])
            ep_m.append(mask)
            ep_a.append(act)
            ep_lp.append(lp)
            ep_r.append(rew)
            ep_v.append(val)

            obs, mask = next_obs, next_mask

        if info.get("winner") == "agent":
            wins += 1
        ep_rewards.append(sum(ep_r))

        if trace_this:
            last_trace_lines.append("\n" + "=" * 88)
            last_trace_lines.append(
                f" GAME OVER ON TURN {info.get('turns')}: Winner = {info.get('winner').upper()} | "
                f"Agent HP: {info.get('agent_hp')} vs Opp HP: {info.get('opp_hp')} | "
                f"Total Episode Reward: {sum(ep_r):+.2f}"
            )
            last_trace_lines.append("=" * 88 + "\n")

        # GAE calculation
        T = len(ep_r)
        adv = np.zeros(T, dtype=np.float32)
        last_gae = 0.0
        for t in reversed(range(T)):
            next_val = 0.0 if t == T - 1 else ep_v[t + 1]
            delta = ep_r[t] + gamma * next_val - ep_v[t]
            last_gae = delta + gamma * gae_lambda * last_gae
            adv[t] = last_gae
        ret = adv + np.asarray(ep_v, dtype=np.float32)

        p_vecs.extend(ep_p)
        c_ids.extend(ep_c)
        n_feats.extend(ep_n)
        masks.extend(ep_m)
        actions.extend(ep_a)
        old_log_probs.extend(ep_lp)
        returns.extend(ret)
        advantages.extend(adv)

    adv_t = torch.as_tensor(np.asarray(advantages), dtype=torch.float32)
    if adv_t.numel() > 1:
        adv_t = (adv_t - adv_t.mean()) / (adv_t.std() + 1e-8)

    return {
        "player_vec": torch.as_tensor(np.asarray(p_vecs), dtype=torch.float32),
        "slot_card_ids": torch.as_tensor(np.asarray(c_ids), dtype=torch.long),
        "slot_num_feats": torch.as_tensor(np.asarray(n_feats), dtype=torch.float32),
        "action_mask": torch.as_tensor(np.asarray(masks), dtype=torch.bool),
        "actions": torch.as_tensor(np.asarray(actions), dtype=torch.long),
        "old_log_probs": torch.as_tensor(np.asarray(old_log_probs), dtype=torch.float32),
        "returns": torch.as_tensor(np.asarray(returns), dtype=torch.float32),
        "advantages": adv_t,
        "win_rate": wins / episodes,
        "mean_reward": float(np.mean(ep_rewards)),
        "num_steps": len(actions),
        "last_trace": "\n".join(last_trace_lines),
    }


def ppo_update(
    model: BattlegroundsMLP,
    optimizer: torch.optim.Optimizer,
    batch: dict[str, torch.Tensor | float | str],
    epochs: int = 2,
    clip_eps: float = 0.2,
    vf_coef: float = 0.5,
    ent_coef: float = 0.01,
) -> dict[str, float]:
    p_vec = batch["player_vec"]
    c_ids = batch["slot_card_ids"]
    n_feats = batch["slot_num_feats"]
    mask = batch["action_mask"]
    actions = batch["actions"]
    old_lp = batch["old_log_probs"]
    returns = batch["returns"]
    advantages = batch["advantages"]

    for _ in range(epochs):
        logits, values = model(p_vec, c_ids, n_feats, mask)
        log_probs = torch.log_softmax(logits, dim=-1)
        probs = torch.exp(log_probs)
        new_lp = log_probs.gather(1, actions.unsqueeze(-1)).squeeze(-1)
        entropy = -(probs * log_probs.clamp(min=-20.0)).sum(dim=-1).mean()

        ratio = torch.exp(new_lp - old_lp)
        surr1 = ratio * advantages
        surr2 = torch.clamp(ratio, 1.0 - clip_eps, 1.0 + clip_eps) * advantages
        policy_loss = -torch.min(surr1, surr2).mean()
        value_loss = nn.functional.mse_loss(values, returns)

        loss = policy_loss + vf_coef * value_loss - ent_coef * entropy
        optimizer.zero_grad(set_to_none=True)
        loss.backward()
        nn.utils.clip_grad_norm_(model.parameters(), 1.0, foreach=True)
        optimizer.step()

    return {
        "policy_loss": float(policy_loss.item()),
        "value_loss": float(value_loss.item()),
        "entropy": float(entropy.item()),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Train MVP BattlegroundsMLP agent via masked PPO.")
    parser.add_argument("--iterations", type=int, default=15)
    parser.add_argument("--episodes-per-iter", type=int, default=2)
    parser.add_argument("--opponent", choices=["random", "heuristic", "self"], default="random")
    parser.add_argument("--lr", type=float, default=5e-4)
    parser.add_argument("--epochs", type=int, default=1)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--dump-last-game", type=str, default=None)
    args = parser.parse_args()

    torch.set_num_threads(1)
    torch.manual_seed(args.seed)
    np.random.seed(args.seed)

    model = BattlegroundsMLP(seed=args.seed)
    env = BattlegroundsEnv(opponent=args.opponent, seed=args.seed, model=model)
    optimizer = torch.optim.Adam(model.parameters(), lr=args.lr, foreach=True)

    pre_r = evaluate_policy(model, opponent="random", episodes=20, seed=90_000)
    pre_h = evaluate_policy(model, opponent="heuristic", episodes=20, seed=95_000)
    print(
        f"[Init Eval] vs random={pre_r['win_rate']:.1%} (turns={pre_r['avg_turns']:.1f}) | "
        f"vs heuristic={pre_h['win_rate']:.1%} (turns={pre_h['avg_turns']:.1f})",
        flush=True,
    )

    log_every = max(1, args.iterations // 20)
    games_per_iter = max(1, args.episodes_per_iter // 2)
    game_counter = 0

    t0 = time.time()
    for it in range(1, args.iterations + 1):
        rec = bool(args.dump_last_game) and (it == args.iterations)
        if args.opponent == "self":
            batch = collect_selfplay_rollouts(
                model,
                games_per_iter=games_per_iter,
                seed_base=args.seed + game_counter * 2,
                game_counter_start=game_counter,
                record_last_game=rec,
            )
            game_counter += games_per_iter
        else:
            batch = collect_rollouts(
                env, model, episodes=args.episodes_per_iter, record_last_episode=rec
            )
        metrics = ppo_update(model, optimizer, batch, epochs=args.epochs)
        if it == 1 or it % log_every == 0 or it == args.iterations:
            elapsed = time.time() - t0
            print(
                f"Iter {it:05d}/{args.iterations:05d} | "
                f"win={batch['win_rate']:.1%} | "
                f"mean_rew={batch['mean_reward']:+.2f} | "
                f"steps={batch['num_steps']:4d} | "
                f"pi_loss={metrics['policy_loss']:+.3f} vf_loss={metrics['value_loss']:.3f} "
                f"ent={metrics['entropy']:.2f} | {elapsed:.1f}s",
                flush=True,
            )
        if rec and batch["last_trace"]:
            with open(args.dump_last_game, "w", encoding="utf-8") as f:
                f.write(str(batch["last_trace"]))
            print(f"Wrote last game trace to {args.dump_last_game}", flush=True)

    post_r = evaluate_policy(model, opponent="random", episodes=20, seed=90_000)
    post_h = evaluate_policy(model, opponent="heuristic", episodes=20, seed=95_000)
    print(
        f"[Final Eval] vs random={post_r['win_rate']:.1%} (turns={post_r['avg_turns']:.1f}) | "
        f"vs heuristic={post_h['win_rate']:.1%} (turns={post_h['avg_turns']:.1f})",
        flush=True,
    )


if __name__ == "__main__":
    main()

