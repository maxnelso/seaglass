"""End-to-end tests for the Rust<->Python seam, bots, environment, and model."""

from __future__ import annotations

import numpy as np
import seaglass
import torch

from seaglass_rl import (
    ACTION_SPACE_SIZE,
    NUM_SLOTS,
    PLAYER_DIM,
    SLOT_NUM_DIM,
    VOCAB_SIZE,
    BattlegroundsEnv,
    BattlegroundsMLP,
    HeuristicBot,
    RandomBot,
    action_from_index,
    action_mask,
    encode_observation,
)
from seaglass_rl.actions import MAX_TRINKET_OPTIONS
from seaglass_rl.features import MAX_TRINKETS


def test_action_mask_matches_engine_legality() -> None:
    game = seaglass.TavernGame(seed=7, catalog="full", deity="cthun")
    game.start_turn()
    mask = action_mask(game, allow_reposition=True, allow_freeze=True)
    assert mask.shape == (ACTION_SPACE_SIZE,)
    for idx, is_valid in enumerate(mask):
        act = action_from_index(idx)
        assert bool(is_valid) == game.is_legal(act)


def test_action_mask_includes_spells_cast_on_the_tavern() -> None:
    # With an empty board, a bought spell that targets "a minion" can only be cast in the Tavern.
    for seed in range(100):
        game = seaglass.TavernGame(seed=seed, catalog="full", deity="cthun")
        game.start_turn()
        for i, card in enumerate(game.observe().shop):
            if card.is_spell and game.is_legal(seaglass.TavernAction.buy(i)):
                game.step(seaglass.TavernAction.buy(i))
                break
        mask = action_mask(game, allow_reposition=True, allow_freeze=True)
        for idx, is_valid in enumerate(mask):
            assert bool(is_valid) == game.is_legal(action_from_index(idx))
        if any(action_from_index(idx).kind == "play_on_shop" for idx in np.flatnonzero(mask)):
            return
    raise AssertionError("no seed offered a spell that can be cast on a minion in the Tavern")


def _play_until_a_golden_is_in_hand(game: seaglass.TavernGame) -> int | None:
    """Buy copies of the board's minions until a triple puts a Golden in the hand."""
    for _ in range(12):
        game.start_turn()
        while True:
            obs = game.observe()
            if obs.discover_pending is not None:
                game.step(seaglass.TavernAction.choose_discover(0))
                continue
            golden = [i for i, c in enumerate(obs.hand) if c.is_golden and not c.is_spell]
            if golden:
                return golden[0]
            owned = {c.card_id for c in obs.board + obs.hand}
            buys = [i for i, c in enumerate(obs.shop) if not c.is_spell and c.card_id in owned]
            buys += [i for i, c in enumerate(obs.shop) if not c.is_spell and len(obs.board) < 6]
            act = next((seaglass.TavernAction.buy(i) for i in buys if game.is_legal(seaglass.TavernAction.buy(i))), None)
            if act is None and obs.hand and len(obs.board) < 6 and not obs.hand[0].is_spell:
                act = seaglass.TavernAction.play(0, len(obs.board))
            if act is None or not game.is_legal(act):
                break
            game.step(act)
        game.end_turn()
    return None


def test_a_golden_played_gives_a_triple_reward_card() -> None:
    # `engine/docs/tavern.md` §5.2: the card's `counter` is its locked-in Discover Tier.
    for seed in range(200):
        # Without trinket offers and Dark Gifts (on by default), which the helper doesn't play.
        game = seaglass.TavernGame(seed=seed, trinkets=False, dark_gifts=False)
        idx = _play_until_a_golden_is_in_hand(game)
        if idx is None or len(game.observe().board) >= 7:
            continue
        obs = game.observe()
        hand_before, tier = len(obs.hand), obs.tavern_tier
        game.step(seaglass.TavernAction.play(idx, len(obs.board)))
        obs = game.observe()
        assert obs.discover_pending is None
        reward = [i for i, c in enumerate(obs.hand) if c.name == "Triple Reward"]
        assert len(reward) == 1 and len(obs.hand) == hand_before
        card = obs.hand[reward[0]]
        assert card.is_spell and card.counter == min(6, tier + 1)
        game.step(seaglass.TavernAction.play(reward[0], 0))
        options = game.observe().discover_pending
        assert options and all(o.tavern_tier == card.counter for o in options)
        return
    raise AssertionError("no seed made a Golden")


def test_encode_observation_shapes_and_vocab() -> None:
    assert VOCAB_SIZE > 300
    game = seaglass.TavernGame(seed=11, catalog="full")
    game.start_turn()
    player_vec, slot_card_ids, slot_num_feats = encode_observation(game.observe())
    assert player_vec.shape == (PLAYER_DIM,)
    assert slot_card_ids.shape == (NUM_SLOTS,)
    assert slot_num_feats.shape == (NUM_SLOTS, SLOT_NUM_DIM)
    assert np.all((slot_card_ids >= 0) & (slot_card_ids < VOCAB_SIZE))


def test_cards_waiting_for_room_are_encoded_last() -> None:
    # `engine/docs/tavern.md` §7.4: a Spellcraft spell given to a full hand waits for room.
    game = seaglass.TavernGame(seed=11, catalog="full")
    game.start_turn()
    obs = game.observe()
    assert obs.waiting_for_hand == []
    player_vec, slot_card_ids, _ = encode_observation(obs)
    # The waiting slots come right before the trinket slots (`engine/docs/trinkets.md` §8.2).
    w = NUM_SLOTS - MAX_TRINKETS - MAX_TRINKET_OPTIONS - 3
    assert player_vec[17] == 0.0 and not slot_card_ids[w : w + 3].any()

    spell = next(u for u in obs.shop if u.is_spell)

    class Waiting:
        waiting_for_hand = [spell]

        def __getattr__(self, name: str) -> object:
            return getattr(obs, name)

    player_vec, slot_card_ids, slot_num_feats = encode_observation(Waiting())
    assert player_vec[17] == np.float32(1.0 / 3.0)
    assert slot_card_ids[w] != 0 and slot_num_feats[w, 4] == 1.0  # a spell
    assert not slot_card_ids[w + 1 : w + 3].any()
    duos = seaglass.DuosTavernGame(seed=3)
    duos.start_turn()
    assert duos.observe(0).waiting_for_hand == []


def test_heuristic_bot_beats_random_bot() -> None:
    heuristic = HeuristicBot()
    h_wins = 0
    games = 20
    for g in range(games):
        p1 = seaglass.TavernGame(seed=100 + g * 2, catalog="full", deity="cthun")
        p2 = seaglass.TavernGame(seed=101 + g * 2, catalog="full", deity="yshaarj")
        rand_bot = RandomBot(seed=500 + g)
        for _ in range(20):
            p1.start_turn()
            p2.start_turn()
            heuristic.play_turn(p1)
            rand_bot.play_turn(p2)
            p1.resolve_combat_with(p2)
            if p1.observe().health <= 0 or p2.observe().health <= 0:
                break
        if p1.observe().health > p2.observe().health:
            h_wins += 1
    assert h_wins >= 14, f"Expected HeuristicBot to win >=14/{games} games vs RandomBot, won {h_wins}"


def test_env_and_mlp_step() -> None:
    env = BattlegroundsEnv(opponent="random", seed=123)
    model = BattlegroundsMLP()
    obs, mask = env.reset()
    done = False
    steps = 0
    while not done:
        action, lp, val = model.act(obs, mask)
        assert np.isfinite(lp) and np.isfinite(val)
        obs, reward, done, mask, info = env.step(action)
        assert np.isfinite(reward)
        steps += 1
    assert steps > 0
    assert info["winner"] in ("agent", "opponent", "draw")


def test_env_episodes_end_with_trinkets_and_dark_gifts() -> None:
    # Every env game has trinkets and Dark Gifts; the agent (random masked actions) and each
    # opponent bot pick trinkets, use the button and resolve their prompts, so episodes end.
    import random

    from seaglass_rl.actions import CHOOSE_TRINKET_INDICES, DARK_DISCOVERY_INDEX
    from seaglass_rl.env import BattlegroundsEnv
    from seaglass_rl.model import BattlegroundsMLP

    for kind in ("random", "heuristic", "self"):
        env = BattlegroundsEnv(opponent=kind, seed=500, model=BattlegroundsMLP(), max_turns=12)
        rng = random.Random(1)
        picked = used = opp_trinkets = 0
        for _ in range(4):
            _, mask = env.reset()
            done, steps = False, 0
            while not done:
                legal = np.flatnonzero(mask)
                assert len(legal) > 0
                a = int(rng.choice(list(legal)))
                picked += a in CHOOSE_TRINKET_INDICES
                used += a == DARK_DISCOVERY_INDEX
                _, _, done, mask, _ = env.step(a)
                steps += 1
                assert steps < 2000, f"{kind}: stuck"
            opp_trinkets += len(env.opp_game.observe().trinkets)
        assert picked > 0 and used > 0 and opp_trinkets > 0, kind
