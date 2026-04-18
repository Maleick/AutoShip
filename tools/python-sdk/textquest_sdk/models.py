"""Data models for the TextQuest API.

All models are pydantic-based for validation and serialization.
"""

from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional
from enum import Enum
from datetime import datetime


@dataclass
class ErrorResponse:
    """API error response."""
    error: str


@dataclass
class HealthResponse:
    """Health check response."""
    status: str
    version: str


@dataclass
class SessionInfo:
    """Session information."""
    session_id: str
    character_name: str
    server: str
    zone: str
    level: int
    class_: str  # class is reserved in Python
    group: int
    online: bool


@dataclass
class Account:
    """Account information."""
    name: str
    server: str
    character: str
    class_: str  # class is reserved in Python
    group: int
    status: str
    has_password: bool = False


@dataclass
class CharacterConfig:
    """Character configuration."""
    character: str
    enabled: bool = True
    rotation: Optional[List[str]] = None
    class_params: Optional[Dict[str, Any]] = None


@dataclass
class EconomySettings:
    """Economy configuration settings."""
    krono_farming_enabled: bool = False
    tradeskill_farming_enabled: bool = False
    restock_interval_minutes: int = 60
    target_wealth_platinum: int = 10000


@dataclass
class VendorRoute:
    """Vendor route for economy."""
    id: str
    name: str
    vendors: List[str] = field(default_factory=list)
    priority: int = 0


@dataclass
class WealthSnapshot:
    """Wealth snapshot at a point in time."""
    total_platinum: int
    character_name: str
    timestamp: Optional[datetime] = None


@dataclass
class WealthHistory:
    """Wealth history data."""
    character: str
    snapshots: List[WealthSnapshot] = field(default_factory=list)


@dataclass
class LootRules:
    """Loot distribution rules."""
    auto_split_stackables: bool = True
    reserve_for_crafting: bool = False
    master_looter_enabled: bool = False


@dataclass
class LootFilters:
    """Character-specific loot filters."""
    character: str
    include_patterns: List[str] = field(default_factory=list)
    exclude_patterns: List[str] = field(default_factory=list)


@dataclass
class SoulState:
    """Soul/character state."""
    character_id: str
    character_name: str
    zone: str
    hp_percent: int
    mana_percent: int
    last_update: Optional[datetime] = None


@dataclass
class AlertConfig:
    """Alert configuration."""
    enabled: bool = True
    alert_on_gm: bool = True
    alert_on_rare_spawn: bool = True
    alert_on_death: bool = True
    discord_webhook: Optional[str] = None


@dataclass
class TimestampConfig:
    """Timestamp configuration for character."""
    character: str
    timestamp_format: str = "%Y-%m-%d %H:%M:%S"
    enabled: bool = True


@dataclass
class PlayerWatchConfig:
    """Player watch configuration."""
    watch_list: List[str] = field(default_factory=list)
    enabled: bool = True


@dataclass
class AutoAcceptSettings:
    """Auto-accept group invite settings."""
    enabled: bool = False
    accept_all: bool = False
    group_leaders: List[str] = field(default_factory=list)


@dataclass
class KillTrackerEntry:
    """Kill tracker entry."""
    character: str
    npc_name: str
    zone: str
    timestamp: Optional[datetime] = None
    loot_value: int = 0


@dataclass
class XAssistConfig:
    """XAssist configuration."""
    character: str
    enabled: bool = False
    assist_target: Optional[str] = None
    assist_range: int = 100


@dataclass
class GmAlertState:
    """GM alert state."""
    gm_detected: bool = False
    gm_names: List[str] = field(default_factory=list)
    last_detected: Optional[datetime] = None


@dataclass
class SpawnAlertState:
    """Spawn alert state."""
    watch_list: List[str] = field(default_factory=list)
    alerts: List[str] = field(default_factory=list)
    last_update: Optional[datetime] = None


@dataclass
class SayDetectionState:
    """Say detection configuration."""
    enabled: bool = False
    channels: List[str] = field(default_factory=list)


@dataclass
class ChatPatternRule:
    """Chat pattern matching rule."""
    id: str
    pattern: str
    action: str
    enabled: bool = True
    cooldown_seconds: int = 0


class DiscordState:
    """Discord webhook state."""
    pass
