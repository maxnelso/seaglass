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


def test_action_mask_matches_engine_legality() -> None:
    game = seaglass.TavernGame(seed=7, catalog="full", deity="cthun")
    game.start_turn()
    mask = action_mask(game, allow_reposition=True, allow_freeze=True)
    assert mask.shape == (ACTION_SPACE_SIZE,)
    for idx, is_valid in enumerate(mask):
        act = action_from_index(idx)
        assert bool(is_valid) == game.is_legal(act)


def test_encode_observation_shapes_and_vocab() -> None:
    assert VOCAB_SIZE > 300
    game = seaglass.TavernGame(seed=11, catalog="full")
    game.start_turn()
    player_vec, slot_card_ids, slot_num_feats = encode_observation(game.observe())
    assert player_vec.shape == (PLAYER_DIM,)
    assert slot_card_ids.shape == (NUM_SLOTS,)
    assert slot_num_feats.shape == (NUM_SLOTS, SLOT_NUM_DIM)
    assert np.all((slot_card_ids >= 0) & (slot_card_ids < VOCAB_SIZE))


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
