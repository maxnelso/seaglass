"""Stage 5's gate: nothing on the training path names a card (`docs/neural-opponents-sprint.md`).

The hand-written bots (`bots.py`, `gamerbot.py`, `bots/`) stay, as opponents to measure against and as
optional scaffolding (the warm start, rule-bot seats). The pure path (`train_lobby --pure`) uses none
of them, and the modules it runs hold no card's name.
"""

from __future__ import annotations

import ast
import random
from pathlib import Path

import pytest
import seaglass

from seaglass_rl import train_lobby
from seaglass_rl.play import BOTS, LEARNER, LobbyConfig

RL = Path(__file__).resolve().parents[1] / "seaglass_rl"
# The modules `train_lobby --pure` trains with: play, encoding, the network, PPO and the trainer.
# The ladder, the probe and the bots only evaluate; `pretrain.py` (the warm start) and `distill.py`
# are not on the pure path.
TRAINING_PATH = (
    "train_lobby.py",
    "play.py",
    "ppo.py",
    "model.py",
    "transformer.py",
    "features.py",
    "card_features.py",
    "actions.py",
    "packing.py",
    "checkpoint.py",
    "device.py",
    "behavior.py",
    "search.py",
)


def _names() -> set[str]:
    names = {t.name for t in seaglass.catalog("full")}
    names |= {row[1] for row in seaglass.heroes_table()}
    names |= {row[1] for row in seaglass.trinkets_table()}
    names |= {row[1] for row in seaglass.dark_gifts_table()}
    # Very short names ("Imp") would match ordinary words.
    return {n for n in names if isinstance(n, str) and len(n) >= 5}


def _strings(path: Path) -> list[str]:
    tree = ast.parse(path.read_text())
    return [node.value for node in ast.walk(tree) if isinstance(node, ast.Constant) and isinstance(node.value, str)]


@pytest.mark.parametrize("module", TRAINING_PATH)
def test_no_card_is_named_on_the_training_path(module: str) -> None:
    names = _names()
    assert len(names) > 300
    hits = sorted({n for s in _strings(RL / module) for n in names if n in s})
    assert not hits, f"{module} names cards: {hits}"


def test_the_check_would_catch_a_named_card(tmp_path) -> None:
    name = next(iter(sorted(_names())))
    f = tmp_path / "bad.py"
    f.write_text(f'FAVOURITE = "{name}"\n')
    assert any(name in s for s in _strings(f))


def test_pure_games_have_no_rule_bots() -> None:
    config = LobbyConfig()
    rng = random.Random(0)
    first = train_lobby.make_specs(config, 8, 4, [], [], 0.0, rng, seed=1)
    assert all(c == LEARNER for spec in first for c in spec.controllers)
    pool = [f"snap_{i}.pt" for i in range(6)]
    later = train_lobby.make_specs(config, 16, 4, pool, ["9kgamerbot", "9kgamerbot-tempo"], 0.0, rng, seed=1)
    for spec in later:
        assert not any(c in BOTS for c in spec.controllers)
        assert sum(c == LEARNER for c in spec.controllers) == 4


@pytest.mark.parametrize(
    "extra", [["--init", "x.pt"], ["--p-bot", "0.1"], ["--auto-position"], ["--aux-coef", "0.1"]]
)
def test_pure_refuses_rule_bot_play(extra: list[str], tmp_path) -> None:
    with pytest.raises(SystemExit):
        train_lobby.main(["--run-dir", str(tmp_path / "run"), "--pure", "--iterations", "1", *extra])
