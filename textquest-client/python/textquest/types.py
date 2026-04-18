from dataclasses import dataclass
from typing import Optional, List, Any, Dict
from enum import Enum


class TextQuestError(Exception):
    def __init__(self, message: str, status_code: Optional[int] = None, response: Optional[Dict[str, Any]] = None):
        super().__init__(message)
        self.message = message
        self.status_code = status_code
        self.response = response


@dataclass
class SessionInfo:
    client_id: int
    character_name: str
    zone: str
    level: int
    hp_pct: float
    mana_pct: float
    endurance_pct: float
    status: str
    buff_count: int
    target_name: Optional[str] = None
    target_hp_pct: Optional[float] = None
    pet_name: Optional[str] = None


@dataclass
class RotationEntry:
    id: str
    name: str
    priority: int
    enabled: bool


@dataclass
class ClassParams:
    ch_chain_timing_ms: Optional[int] = None
    dot_overlap_pct: Optional[int] = None
    burn_at_hp_pct: Optional[int] = None
    slow_at_hp_pct: Optional[int] = None


@dataclass
class CharacterConfig:
    character_name: str
    class_name: str
    role: str
    heal_at_pct: int
    mana_sit_pct: int
    nuke_at_pct: int
    rotation: List[RotationEntry]
    class_params: ClassParams
    group_override: bool
    group_name: Optional[str] = None


@dataclass
class HealthResponse:
    status: str
    version: str


@dataclass
class ErrorResponse:
    error: str


@dataclass
class CommandRequest:
    command: str
    target: Optional[str] = None


@dataclass
class CommandResponse:
    success: bool
    message: str


@dataclass
class GroupAssignment:
    group_id: int


class SessionEventType(Enum):
    SESSION_UPDATE = "session_update"
    COMMAND = "command"
    ERROR = "error"


@dataclass
class SessionEvent:
    type: SessionEventType
    data: Dict[str, Any]