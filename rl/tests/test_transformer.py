"""The entity-transformer policy (`transformer.py`): its interface matches `BattlegroundsMLP`'s, its
action head moves with the cards, and it trains with PPO and behavior cloning."""

from __future__ import annotations

import numpy as np
import pytest
import torch

from seaglass_rl import checkpoint
from seaglass_rl.actions import ACTION_KIND_IDS, ACTION_SPACE_SIZE, ACTION_TO_INDEX, action_mask
from seaglass_rl.features import NUM_SLOTS, PLAYER_DIM, SLOT_NUM_DIM, encode_observation
from seaglass_rl.play import GameSpec, LobbyConfig, PlayOptions, Seat, play_games
from seaglass_rl.ppo import build_batch, ppo_update
from seaglass_rl.transformer import HAND_OFFSET, EntityTransformer, _action_table

SMALL = dict(d_model=32, layers=2, heads=2, ff=64, head_dim=32, card_embed_dim=8)


@pytest.fixture(scope="module")
def observations() -> dict[str, np.ndarray]:
    """Real decisions from a short HeuristicBot lobby."""
    (res,) = play_games(
        LobbyConfig(players=4, max_turns=8), [GameSpec(seed=11, controllers=["heuristic"] * 4)], {}, record=("heuristic",)
    )
    keys = ("player_vec", "slot_card_ids", "slot_num_feats", "action_mask")
    return {k: np.concatenate([t[k] for t in res.trajectories]) for k in keys}


def _tensors(obs: dict[str, np.ndarray], idx) -> tuple[torch.Tensor, ...]:
    return (
        torch.from_numpy(obs["player_vec"][idx]),
        torch.from_numpy(obs["slot_card_ids"][idx].astype(np.int64)),
        torch.from_numpy(obs["slot_num_feats"][idx].astype(np.float32)),
        torch.from_numpy(obs["action_mask"][idx]),
    )


def _random_model(seed: int = 0, **kw) -> EntityTransformer:
    """A model with the output layer scaled up, so the logits differ visibly."""
    torch.manual_seed(seed)
    model = EntityTransformer(seed=seed, **{**SMALL, **kw})
    with torch.no_grad():
        model.actor_out.weight.normal_(std=1.0)
    return model.eval()


def test_shapes_and_masked_actions_get_no_probability(observations) -> None:
    model = _random_model()
    pv, ci, nf, mk = _tensors(observations, slice(0, 16))
    assert pv.shape[1] == PLAYER_DIM and ci.shape[1] == NUM_SLOTS and nf.shape[2] == SLOT_NUM_DIM
    logits, value = model(pv, ci, nf, mk)
    assert logits.shape == (16, ACTION_SPACE_SIZE) and value.shape == (16,)
    probs = torch.softmax(logits, -1)
    assert torch.all(probs[~mk] == 0)
    assert torch.allclose(probs.sum(-1), torch.ones(16))
    # Without a mask: every logit is finite.
    raw, _ = model(pv, ci, nf)
    assert torch.isfinite(raw).all()


def test_act_batch_matches_forward(observations) -> None:
    model = _random_model(1)
    idx = slice(0, 24)
    pv, ci, nf, mk = _tensors(observations, idx)
    logits, value = model(pv, ci, nf, mk)
    lp = torch.log_softmax(logits, -1)
    np_in = [observations[k][idx] for k in ("player_vec", "slot_card_ids", "slot_num_feats", "action_mask")]
    acts, lps, vals = model.act_batch(*np_in)
    assert observations["action_mask"][idx][np.arange(24), acts].all()
    assert np.allclose(lps, lp[torch.arange(24), torch.from_numpy(acts)].detach().numpy(), atol=1e-4)
    assert np.allclose(vals, value.detach().numpy(), atol=1e-4)
    acts, lps, _ = model.act_batch(*np_in, deterministic=True)
    # Identical cards in symmetric slots (e.g. two copies in the shop) tie up to float32 rounding.
    best = lp.max(-1).values.detach().numpy()
    assert np.allclose(lp[torch.arange(24), torch.from_numpy(acts)].detach().numpy(), best, atol=1e-5)
    assert np.allclose(lps, best, atol=1e-4)
    # A row's outputs don't depend on the rest of its batch (the packing pads to the fullest row).
    _, lps_one, vals_one = model.act_batch(*(x[:1] for x in np_in), deterministic=True)
    assert np.isclose(lps_one[0], lps[0], atol=1e-4) and np.isclose(vals_one[0], vals[0], atol=1e-4)
    # The cached card tokens follow the weights (an optimizer step changes them in place).
    with torch.no_grad():
        model.card_in.weight.mul_(1.5)
        model.card_embed.weight.add_(0.1)
    logits, value = model(pv, ci, nf, mk)
    _, lps, vals = model.act_batch(*np_in, deterministic=True)
    assert np.allclose(lps, torch.log_softmax(logits, -1).max(-1).values.detach().numpy(), atol=1e-4)
    assert np.allclose(vals, value.detach().numpy(), atol=1e-4)


def test_scoring_only_the_legal_actions_and_bucketing_change_nothing(observations, monkeypatch) -> None:
    model = _random_model(4)
    pv, ci, nf, mk = _tensors(observations, slice(0, 40))
    logits, value = model(pv, ci, nf, mk)
    raw, raw_value = model(pv, ci, nf)
    assert torch.allclose(logits[mk], raw[mk], atol=1e-5) and torch.allclose(value, raw_value, atol=1e-5)
    # Buckets of 3 rows each pad to their own longest row: the same outputs.
    monkeypatch.setattr(EntityTransformer, "MIN_BUCKET_ROWS", 3)
    monkeypatch.setattr(EntityTransformer, "BUCKETS", 100)
    small_logits, small_value = model(pv, ci, nf, mk)
    assert torch.allclose(small_logits, logits, atol=1e-4) and torch.allclose(small_value, value, atol=1e-5)


def test_checkpoints_round_trip(observations, tmp_path) -> None:
    model = _random_model(2, kind_prior=True)
    path = tmp_path / "tf.pt"
    checkpoint.save(path, model, iteration=3)
    loaded, ckpt = checkpoint.load(path)
    assert isinstance(loaded, EntityTransformer) and loaded.config == model.config
    assert "static_table" not in ckpt["model_state"]
    batch = _tensors(observations, slice(0, 8))
    for a, b in zip(model(*batch), loaded(*batch)):
        assert torch.equal(a, b)
    # `play.py`'s `(config, state_dict)` sources build the right class too.
    rebuilt = checkpoint.model_from_config(dict(model.config))
    rebuilt.load_state_dict(model.state_dict())
    assert isinstance(rebuilt, EntityTransformer)


def test_swapping_two_hand_cards_swaps_their_play_and_pass_logits(observations) -> None:
    model = _random_model(3)
    hand = observations["slot_card_ids"][:, HAND_OFFSET : HAND_OFFSET + 10] != 0
    row = int(np.flatnonzero(hand.sum(1) >= 2)[0])
    a, b = np.flatnonzero(hand[row])[:2]
    pv, ci, nf, _ = _tensors(observations, [row])
    ci2, nf2 = ci.clone(), nf.clone()
    sa, sb = HAND_OFFSET + a, HAND_OFFSET + b
    ci2[0, [sa, sb]] = ci[0, [sb, sa]]
    nf2[0, [sa, sb]] = nf[0, [sb, sa]]
    logits, value = model(pv, ci, nf)
    logits2, value2 = model(pv, ci2, nf2)
    assert torch.allclose(value, value2, atol=1e-5)
    # Hand slot `h`'s actions, in a fixed order.
    def hand_actions(h: int) -> list[int]:
        plays = [ACTION_TO_INDEX[("play", h, p)] for p in range(7)]
        on_shop = [ACTION_TO_INDEX[("play_on_shop", h, s)] for s in range(7)]
        return plays + on_shop + [ACTION_TO_INDEX[("pass", h, None)]]

    for x, y in ((a, b), (b, a)):
        assert torch.allclose(logits2[0, hand_actions(x)], logits[0, hand_actions(y)], atol=1e-5)
    # The two cards differ, so their logits do too (the check above is not vacuous).
    assert not torch.allclose(logits[0, hand_actions(a)], logits[0, hand_actions(b)], atol=1e-3)
    # Everything else stays put.
    others = sorted(set(range(ACTION_SPACE_SIZE)) - set(hand_actions(a)) - set(hand_actions(b)))
    assert torch.allclose(logits2[0, others], logits[0, others], atol=1e-5)


def test_the_kind_prior_spreads_a_zeroed_actor_over_kinds() -> None:
    from seaglass_rl.actions import action_from_index
    from seaglass_rl.bots import HeuristicBot

    game = LobbyConfig(players=2, heroes=False).make(1)
    bot = HeuristicBot()
    for _ in range(3):  # a few HeuristicBot turns, so the Tavern has some cards
        game.begin_phase()
        if game.phase == "turn":
            for p in game.active_players():
                seat = Seat(game, p)
                for _ in range(25):
                    if game.is_done(p):
                        break
                    seat.step(action_from_index(bot.act(seat)))
        game.end_phase()
    game.begin_phase()
    seat = Seat(game, 0)
    obs = seat.observe()
    enc = encode_observation(obs, game.context(0))
    mask = action_mask(seat, allow_reposition=True, allow_freeze=True, obs=obs)
    model = EntityTransformer(seed=0, kind_prior=True, **SMALL)
    with torch.no_grad():
        model.actor_out.weight.zero_()
        model.actor_out.bias.zero_()
        logits, value = model(*(torch.from_numpy(x[None]) for x in enc), torch.from_numpy(mask[None]))
    probs = torch.softmax(logits, -1)[0].numpy()
    kinds = set(ACTION_KIND_IDS[mask])
    assert len(kinds) >= 2
    per_kind = [probs[ACTION_KIND_IDS == k].sum() for k in kinds]
    assert np.allclose(per_kind, 1 / len(kinds), atol=1e-5)


def test_action_table_points_each_action_at_its_cards() -> None:
    table = _action_table()
    play = ACTION_TO_INDEX[("play", 3, 5)]
    assert table["src"][play] == HAND_OFFSET + 3 and table["pos"][play] == 6 and table["tgt"][play] == -1
    on_shop = ACTION_TO_INDEX[("play_on_shop", 2, 4)]
    assert table["src"][on_shop] == HAND_OFFSET + 2 and table["tgt"][on_shop] == HAND_OFFSET + 10 + 4
    assert table["src"][ACTION_TO_INDEX[("end_turn", None, None)]] == NUM_SLOTS


@pytest.mark.parametrize("duos", [False, True])
def test_the_transformer_plays_trains_and_clones(duos: bool) -> None:
    from seaglass_rl.pretrain import collect, train

    config = LobbyConfig(players=4, duos=duos, max_turns=8)
    model = EntityTransformer(seed=0, kind_prior=True, **SMALL)
    controllers = ["learner", "heuristic", "learner", "heuristic"][: config.units]
    results = play_games(config, [GameSpec(seed=4, controllers=controllers)], {"learner": model}, PlayOptions(shaping=0.1))
    batch = build_batch([t for r in results for t in r.trajectories], gamma=0.97, lam=0.95)
    opt = torch.optim.Adam(model.parameters(), lr=1e-3)
    metrics = ppo_update(model, opt, batch, epochs=2, minibatch_size=128)
    assert np.isfinite(metrics["policy_loss"]) and metrics["epochs"] >= 1
    data = collect(config, games=1, workers=0, options=PlayOptions(shaping=0.3), seed=5)
    stats = train(model, data, epochs=1, lr=3e-3, minibatch=128, holdout=0.2)
    assert 0.0 <= stats["accuracy"] <= 1.0 and np.isfinite(stats["value_mse"])
