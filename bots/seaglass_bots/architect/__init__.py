"""ArchitectBot: Composition-Driven Superhuman Search & Execution Bot (Patch 37.0.2)."""

from .alpha_ppo_bot import PPOAlphaSearchBot
from .bot import ArchitectBot as PureArchitectBot
from .comps import ALL_COMPS, CompSpec, get_comp_registry
from .evaluator import ArchitectEvaluator
from .hybrid import (
    DEFAULT_ARCHITECT_NPZ_PATH,
    ArchitectBot,
    ArchitectHybridEvaluator,
    ArchitectHybridPositioner,
)
from .planner import BuildPlan, CompPlanner, CompScore
from .policy_bot import DEFAULT_POLICY_PT_PATH, NeuralPolicyBot, PolicyZeroBot
from .positioning import ArchitectPositioner
from .zero_bot import ZeroPhysicsBot

__all__ = [
    "ArchitectBot",
    "PureArchitectBot",
    "ArchitectHybridEvaluator",
    "ArchitectHybridPositioner",
    "ArchitectEvaluator",
    "ArchitectPositioner",
    "ZeroPhysicsBot",
    "NeuralPolicyBot",
    "PolicyZeroBot",
    "PPOAlphaSearchBot",
    "ALL_COMPS",
    "CompSpec",
    "CompPlanner",
    "CompScore",
    "BuildPlan",
    "get_comp_registry",
    "DEFAULT_ARCHITECT_NPZ_PATH",
    "DEFAULT_POLICY_PT_PATH",
]
