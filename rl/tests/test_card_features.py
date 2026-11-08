"""The static per-card features (`seaglass_rl.card_features`) and the MLP that uses them."""

from __future__ import annotations

import numpy as np
import torch

from seaglass_rl.card_features import STATIC_DIM, TAG_DIM, TAG_NAMES, TEXT_DIM, static_card_table
from seaglass_rl.features import CARD_ID_TO_IDX, NUM_SLOTS, PLAYER_DIM, SLOT_NUM_DIM, VOCAB_SIZE
from seaglass_rl.actions import ACTION_SPACE_SIZE
from seaglass_rl.model import BattlegroundsMLP


def _tag(name: str) -> np.ndarray:
    return static_card_table()[:, TAG_NAMES.index(name)]


def test_table_shape_and_empty_row():
    table = static_card_table()
    assert table.shape == (VOCAB_SIZE, STATIC_DIM) == (VOCAB_SIZE, TAG_DIM + TEXT_DIM)
    assert table.dtype == np.float32
    assert not table[0].any(), "row 0 is 'no card'"


def test_tags_tell_effects_apart():
    battlecry, deathrattle = _tag("battlecry"), _tag("deathrattle")
    assert battlecry.sum() > 10 and deathrattle.sum() > 10
    only_bc = np.flatnonzero((battlecry == 1) & (deathrattle == 0))
    only_dr = np.flatnonzero((deathrattle == 1) & (battlecry == 0))
    assert len(only_bc) and len(only_dr)
    table = static_card_table()
    assert not np.array_equal(table[only_bc[0], :TAG_DIM], table[only_dr[0], :TAG_DIM])


def test_minion_template_tags():
    minion = _tag("minion")
    tiers = static_card_table()[:, [TAG_NAMES.index(f"tier_{t}") for t in range(1, 8)]]
    # Every minion has exactly one Tavern Tier, and only minions have one.
    np.testing.assert_array_equal(tiers.sum(1), minion)
    assert minion.sum() > 200
    assert (_tag("health")[minion == 1] > 0).all()
    assert _tag("taunt").sum() > 5 and _tag("divine_shield").sum() > 5
    assert _tag("tribe_beast").sum() > 10


def test_most_cards_have_a_text_embedding():
    text = static_card_table()[1:, TAG_DIM:]
    has_text = np.abs(text).sum(1) > 0
    assert has_text.mean() > 0.9, f"only {has_text.mean():.0%} of the vocabulary has a text embedding"


def test_cards_without_any_tag_are_rare():
    tags = static_card_table()[list(CARD_ID_TO_IDX.values()), :TAG_DIM]
    assert (tags.sum(1) == 0).mean() < 0.1


def _batch(rng: np.random.Generator, b: int):
    p = rng.normal(size=(b, PLAYER_DIM)).astype(np.float32)
    c = rng.integers(0, VOCAB_SIZE, size=(b, NUM_SLOTS)).astype(np.int64)
    n = rng.normal(size=(b, NUM_SLOTS, SLOT_NUM_DIM)).astype(np.float32)
    m = rng.random((b, ACTION_SPACE_SIZE)) < 0.3
    m[:, 0] = True
    return p, c, n, m


def test_mlp_static_features_forward_matches_act_batch():
    torch.manual_seed(0)
    model = BattlegroundsMLP(card_embed_dim=8, hidden_dim=32, static_features=True)
    assert model.static_features
    p, c, n, m = _batch(np.random.default_rng(0), 5)
    acts, lps, vals = model.act_batch(p, c, n, m, deterministic=True)
    with torch.no_grad():
        logits, value = model(torch.from_numpy(p), torch.from_numpy(c), torch.from_numpy(n), torch.from_numpy(m))
    log_probs = torch.log_softmax(logits, dim=-1).numpy()
    np.testing.assert_array_equal(acts, logits.argmax(-1).numpy())
    np.testing.assert_allclose(lps, log_probs[np.arange(5), acts], atol=1e-4)
    np.testing.assert_allclose(vals, value.numpy(), atol=1e-4)


def test_mlp_static_table_is_not_saved_and_reloads():
    model = BattlegroundsMLP(card_embed_dim=8, hidden_dim=32, static_features=True)
    state = model.state_dict()
    assert "static_table" not in state and "static_proj.weight" in state
    clone = BattlegroundsMLP(**model.config)
    clone.load_state_dict(state)
    p, c, n, m = _batch(np.random.default_rng(1), 3)
    args = [torch.from_numpy(x) for x in (p, c, n, m)]
    with torch.no_grad():
        torch.testing.assert_close(model(*args), clone(*args))


def test_static_features_make_similar_cards_start_similar():
    """With the embeddings zeroed, two cards' vectors differ only through their static rows."""
    model = BattlegroundsMLP(card_embed_dim=8, hidden_dim=32, static_features=True)
    with torch.no_grad():
        model.card_embed.weight.zero_()
        table = model.card_table()
    static = torch.from_numpy(static_card_table())
    same = [i for i in range(1, VOCAB_SIZE) if torch.equal(static[i], static[1])]
    for i in same:
        torch.testing.assert_close(table[i], table[1])
    assert not table[0].any()


def test_sampling_never_picks_a_masked_action():
    """Float32 cumulative sums can fall short of 1: the sampler must still stay on legal actions
    (the last action index, `dark_discovery`, was once picked when illegal)."""
    from seaglass_rl.model import sample_categorical

    rng = np.random.default_rng(0)
    probs = np.zeros((20000, ACTION_SPACE_SIZE), dtype=np.float32)
    probs[:, :3] = np.float32(1 / 3)  # sums to slightly less than 1 in float32
    acts = sample_categorical(probs, rng)
    assert acts.max() < 3
    assert set(np.unique(acts)) == {0, 1, 2}
    # A row whose only legal action is the first one.
    one = np.zeros((1000, ACTION_SPACE_SIZE), dtype=np.float32)
    one[:, 0] = 1.0
    assert (sample_categorical(one, rng) == 0).all()
