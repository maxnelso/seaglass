# Seaglass RL (`rl/`)

`seaglass-rl` is the Python reinforcement learning project for training agents to play Hearthstone Battlegrounds on top of the [`seaglass` Rust engine](../engine/README.md).

**All game rules live in Rust; all RL decisions live in Python.**
This package never reimplements a card, cost, legality check, or combat step—it calls the `seaglass` native extension for state transitions and batch combat simulations, while keeping the MDP formulation (observation encoding, action space mapping, reward shaping, and opponent curriculum) and neural network training in Python for rapid iteration.

---

## Responsibilities of `rl/`

1. **RL Environment (`env.py`)**
   Wraps the Rust engine's tavern state machine and batch combat simulator into a single-player or multi-agent RL environment (`BattlegroundsEnv`) with deterministic seeding.
2. **Action Space & Masking (`actions.py`)**
   Defines the discrete action space consumed by the policy network, queries the engine for legal actions at every micro-step, and produces boolean action masks. Illegal actions raise `ValueError` immediately rather than silently no-oping.
3. **Reward Shaping (`rewards.py`)**
   Computes step and end-of-turn rewards in Python using combat equity from `seaglass.simulate_batch`, tavern tier progression, hero health preservation, and micro-action shaping bonuses.
4. **Opponent Curriculum & Self-Play (`opponents.py`)**
   Generates opponent boards for end-of-turn combat evaluation—ranging from deterministic turn-scaling baseline boards to historical ghost boards and self-play checkpoints.
5. **Feature Encoding & Policy Network (`features.py`, `model.py`)**
   Converts raw engine observations into entity tensors and runs them through a Transformer Actor-Critic (`BattlegroundsActorCritic`).
6. **Training & Inspection (`train_ppo.py`, `sandbox_cli.py`)**
   Masked PPO training loop, evaluation benchmarks, and an interactive CLI inspector.

---

## MDP Formulation

### 1. Observation & Feature Encoding
At each step, the Rust engine returns raw state (`turn`, `health`, `gold`, `max_gold`, `tavern_tier`, `upgrade_cost`, `bonus_gold_next_turn`, `is_frozen`, and `PyUnit` lists for `board`, `hand`, `shop`, and `discover`).

`features.py` encodes this into:
- **Player Token (`[PLAYER]`)**: Normalized vector of global economy and turn state:
  `[gold/10, max_gold/10, tavern_tier/6, upgrade_cost/10, turn/max_turns, health/30, bonus_gold_next_turn/5]`.
- **Card Entity Tokens**: One token per card across `board` (`zone=0`), `hand` (`zone=1`), and `shop` (`zone=2`), containing:
  - `card_id`: Dense vocabulary index derived dynamically at import time from `seaglass.catalog("full")` plus token/spell IDs (never a hardcoded ID table).
  - `tribe`: Engine `Tribe` discriminant (`0..22`; new types such as `naga` are appended, so existing values keep their meaning).
  - `zone`: Zone index (`0..2`).
  - `stats`: `[log1p(attack), log1p(health), tavern_tier / 6, slot / 7]`.
  - `keywords`: Binary flags `[is_golden, taunt, divine_shield, windfury, reborn]`.

### 2. Action Space & Legality Mask
During a Tavern turn, the agent repeatedly takes discrete micro-actions until selecting `EndTurn`.
- Every action index maps deterministically to an engine `TavernAction` (`Buy`, `Play`, `PlayOnShop` (a spell cast on a minion in the Tavern), `Sell`, `Reposition`, `Activate`, `ChooseDiscover`, `Refresh`, `UpgradeTavern`, `ToggleFreeze`, `EndTurn`, the Duos `Pass`, and the heroes' `HeroPower` and `ChooseHero`: see [Heroes](#heroes)). New kinds are appended at the end so existing indices keep their meaning, but the action space grows, so checkpoints saved before a new kind was added don't load.
- The boolean `action_mask` is queried directly from the engine's legality check at every step. When a Discover is pending (the `Triple Reward` card cast, a Discover effect), all actions except `ChooseDiscover` are masked out. A Golden played from the hand gives a `Triple Reward` card (a 0-Gold spell in the hand, `engine/docs/tavern.md` §5.2); its locked-in Discover Tier is the card's `counter` (`seaglass.Unit.counter`), and casting it is a `Play`.

### 3. Reward Function
Default potential-based reward shaping over turn boundaries:

$$\Phi(s) = w_{\text{eq}} \cdot \text{combat\_equity}(s) + w_{\text{tier}} \cdot (\text{tavern\_tier} - 1) + w_{\text{hp}} \cdot \frac{\text{health}}{30}$$

$$r_t = \Phi(s_t) - \Phi(s_{t-1}) + r_{\text{shaping}}$$

where $\text{combat\_equity}(s) = \text{win\_rate} + 0.5 \cdot \text{draw\_rate}$ is estimated via `seaglass.simulate_batch` against the current turn's opponent board. Because $\Phi(s)$ is computed in Python, reward weights and shaping terms can be configured per experiment without touching the Rust crate.

---

## Duos

A Duos team (two players with one Health pool who can Pass cards to each other, [`engine/docs/duos_tavern.md`](../engine/docs/duos_tavern.md)) is `seaglass.DuosTavernGame`. The engine has everything a Duos agent needs; the encoding below and a Duos environment are the next step (the policy network and training are unchanged).

### Bindings
- `DuosTavernGame(seed=0, catalog="duos", armor=(5, 5), first_fighter_switch_chance=0.75, shop_spells=True, deity=None)`: one team's Tavern phase, with its card pool and RNG (everything is deterministic from `seed`). `armor` is each hero's starting Armor (the team starts with 30 Health and their sum); `catalog="duos"` is the full catalog plus the Duos-exclusive minions (`Wanderer Cho`).
- Per player (`0` or `1`): `observe(player)`, `legal_actions(player)`, `is_legal(player, action)`, `step(player, action)` (returns `True` if it ended that player's turn), `end_turn(player)`.
- Per team: `start_turn()` (both players, then the coming combat's first fighter), `turn_over()` (both players ended), `is_out()`, `health`, `armor`, `first_fighter`; `resolve_combat_with(other, seed=None)` (team vs team: both take their results, the loser the damage) and `resolve_combat_against(opponent, first=0, hero_tiers=(1, 1), seed=None)` (a fixed opponent team: its two boards). Both return a `DuosBattleResult` (`outcome`, `hero_damage`, `fought_a`/`_b`, `survivors_a`/`_b`, per player). Two `DuosTavernGame`s have separate card pools (4-team matches are out of scope).
- `TavernAction.pass_card(hand_index)`: kind `"pass"`, args `(hand_index, None)`. A Solo `TavernGame` never lists it as legal.
- `simulate_duos(team_a, team_b, seed, first_a=0, first_b=0, hero_tiers_a=(1, 1), hero_tiers_b=(1, 1))` and `simulate_duos_batch(team_a, team_b, base_seed, n, ...)`: Duos combat equity, where a team is its two players' boards (`[[units], [units]]`).

### MDP
- **Agents**: each player is an agent acting on their own Tavern. Both act during the same turn: the environment interleaves their micro-steps (any order is legal) until both have ended; then the teams fight. Rewards follow the team: its shared Health, the team combat's outcome and damage, and `simulate_duos_batch` equity.
- **Actions**: the Solo table plus 10 `Pass` actions, one per hand slot, appended at the end (`PASS_INDICES`), so Solo indices keep their meaning and Solo masks leave them out. `action_mask(game, ..., player=p)` masks a Duos player's actions.
- **Observation** (`DuosObservation`): the player's own `TavernObservation` fields (with the team's `health` and `armor`), plus `incoming` (the cards passed to them waiting for room in a full hand), `free_passes`, `fights_first`, `ended`; the teammate's `teammate_board`, `teammate_hand`, `teammate_shop`, `teammate_tavern_tier`, `teammate_gold`, `teammate_incoming`, `teammate_ended`, `teammate_free_passes`; and the team's `team_health`, `team_armor`, `passes_this_turn`, `passes_this_game`, `first_fighter`.
- **Planned encoding**: the teammate's cards become extra card tokens with new zones appended after the Solo ones (`3` teammate board, `4` teammate hand, `5` teammate shop, `6` own incoming queue, `7` teammate's incoming queue), and the team scalars (team Health/Armor, free Passes, the Pass counters, `fights_first`, the teammate's Tier and Gold, both `ended` flags) are appended to the `[PLAYER]` vector. The card vocabulary already includes the Duos-exclusive cards: their ids (`10_000` and up) sort after every Solo card, so they only append to it.

---

## Heroes

Heroes and their Hero Powers ([`engine/docs/heroes.md`](../engine/docs/heroes.md)) are optional: a game without a hero (the default) is exactly as before.

### Bindings
- `TavernGame(..., hero=None)`: a hero by name (`"Y'Shaarj"`) or id (its dbfId, `66196`) gives the player its Health, Armor (then `armor` is ignored) and Hero Power. `DuosTavernGame(..., heroes=None)`: a pair `(hero_0, hero_1)` (each a name, an id or `None`); the team gets 30 Health plus both heroes' Duos Armor (Patchwerk: his extra 30 Health), replacing `armor`.
- `TavernAction.hero_power(target=None)` (kind `"hero_power"`, args `(target, None)`): use the Hero Power, on a board, shop or hand index if it needs one. `TavernAction.choose_hero(option_index)` (kind `"choose_hero"`): choose an offered hero; while heroes are offered, it is the only legal action.
- `heroes_table()`: `(id, name, implemented)` for each of the 123 heroes of the pool.

### Observation
`TavernObservation` and `DuosObservation` have `hero_id` (0 without a hero), `hero_power_id` (the Hero Power card in use: it changes with Bru'kan's Element), `hero_power_cost`, `hero_power_uses_left`, `hero_power_counter`, `hero_power_stacks`, `hero_power_armed` (next-combat effects paid for: Y'Shaarj), `hero_power_usable` (some `HeroPower` action is legal now) and `hero_options`; `DuosObservation` also has `teammate_hero_id`. `features.py` appends six of them to the `[PLAYER]` vector (`PLAYER_DIM` is 17 with the next ones): has a hero, `hero_power_usable`, `hero_power_cost / 10`, `hero_power_uses_left`, `hero_power_armed / 3`, and whether heroes are offered. Freezing (`engine/docs/tavern.md` §3.1) is appended after them: `is_frozen` (the whole shop) ends the `[PLAYER]` vector, and each slot's last numeric feature (`SLOT_NUM_DIM` is 13) is `Unit.frozen`, a shop card frozen on its own (Varden Dawngrasp's copies). `ToggleFreeze` unfreezes everything when anything is frozen, and freezes the shop otherwise. Then `next_minion_discount / 3` (the next minion bought costs that much less: Aranna Starseeker's free minion; `PLAYER_DIM` is 17).

### Actions
After the Duos `Pass` actions, the table appends 11 `HeroPower` actions (`HERO_POWER_INDICES`: untargeted, then targets `0..10`) and 4 `ChooseHero` actions (`CHOOSE_HERO_INDICES`; 2 heroes are offered by default). A player without a hero never lists them as legal.

---

## Model Architecture (`BattlegroundsActorCritic`)

1. **`PlayerEntityEncoder`**: Projects the 7-D player economy vector into a 64-D `[PLAYER]` token (`Entity #0`).
2. **`CardEntityEncoder`**: Embeds `card_id`, `tribe`, and `zone` via `nn.Embedding`, concatenates continuous stats and keyword flags, and projects each card to a 64-D token (`Entities #1..N`).
3. **Transformer Backbone**: Stacks `[PLAYER, Card_1, ..., Card_N]` into a `[1, 1 + N, 64]` sequence and passes it through a 2-layer `nn.TransformerEncoder` so cards attend to the player economy and to each other (synergies, triples, tribal scaling).
4. **Heads (read from transformed `Entity #0`)**:
   - **Actor Head**: Linear projection to action logits, masked with `-1e9` wherever `action_mask == False`.
   - **Critic Head**: Linear projection to scalar state-value estimate $V(s)$.

---

## Package Layout

```text
rl/
├── README.md                  # This file
├── pyproject.toml             # Package metadata & dependencies (torch, pytest, maturin)
├── seaglass_rl/
│   ├── __init__.py
│   ├── env.py                 # BattlegroundsEnv (Gym-like interface over `seaglass`)
│   ├── actions.py             # Action space mapping & mask construction
│   ├── features.py            # Dynamic card vocabulary & observation -> tensor encoding
│   ├── rewards.py             # Potential-based reward shaping & combat equity evaluation
│   ├── opponents.py           # Synthetic scaling opponent & self-play opponent pool
│   ├── model.py               # CardEntityEncoder, PlayerEntityEncoder, BattlegroundsActorCritic
│   ├── train_ppo.py           # PPO rollout collection, GAE/returns, clipped policy update
│   └── sandbox_cli.py         # Interactive terminal inspector for tavern & combat
└── tests/
    ├── test_env_and_seam.py   # Verifies Rust<->Python determinism, catalog sync, and masks
    ├── test_duos.py           # A seeded Duos team game driven from Python: Passes, masks, team combat
    └── test_heroes.py         # Heroes: Hero Power actions and masks, observation fields, Duos Armor
```

---

## Quickstart

### 1. Build & install the `seaglass` Rust extension

From the `rl/` directory (inside your Python virtual environment):

```bash
maturin develop --release --manifest-path ../engine/Cargo.toml --features python
```

### 2. Run seam & environment tests

```bash
pytest
```

### 3. Explore interactively in the CLI sandbox

```bash
python -m seaglass_rl.sandbox_cli
```

### 4. Train a PPO agent

```bash
python -m seaglass_rl.train_ppo --iterations 25 --episodes-per-iter 8
```
