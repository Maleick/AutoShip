from __future__ import annotations

from dataclasses import MISSING, asdict, dataclass, fields, is_dataclass
from enum import Enum
from typing import Any, Optional, List, Dict, TypeVar, Union, get_args, get_origin


JSONScalar = str | int | float | bool | None
JSONValue = JSONScalar | list["JSONValue"] | dict[str, "JSONValue"]


def _snake_to_camel(value: str) -> str:
    parts = value.split("_")
    return parts[0] + "".join(part.capitalize() for part in parts[1:])


def _decode_value(annotation: Any, value: Any) -> Any:
    if value is None:
        return None

    origin = get_origin(annotation)
    args = get_args(annotation)

    if origin in (list, tuple, set):
        inner = args[0] if args else Any
        return [_decode_value(inner, item) for item in value]

    if origin is dict:
        key_type, value_type = args if len(args) == 2 else (str, Any)
        return {
            _decode_value(key_type, key): _decode_value(value_type, item)
            for key, item in value.items()
        }

    if origin in (Union, getattr(__import__("types"), "UnionType", Union)):
        non_none = [arg for arg in args if arg is not type(None)]
        if len(non_none) == 1:
            return _decode_value(non_none[0], value)

    if isinstance(annotation, type) and issubclass(annotation, Enum):
        return annotation(value)

    if isinstance(annotation, type) and is_dataclass(annotation):
        return model_from_dict(annotation, value)

    return value


T = TypeVar("T")


def model_from_dict(model_type: type[T], payload: dict[str, Any]) -> T:
    kwargs: dict[str, Any] = {}
    for field in fields(model_type):
        if field.name in payload:
            raw = payload[field.name]
        else:
            alias = _snake_to_camel(field.name)
            if alias in payload:
                raw = payload[alias]
            elif field.default is not MISSING:
                raw = field.default
            elif field.default_factory is not MISSING:  # type: ignore[attr-defined]
                raw = field.default_factory()  # type: ignore[misc]
            else:
                continue
        kwargs[field.name] = _decode_value(field.type, raw)
    return model_type(**kwargs)


def encode_payload(payload: Any) -> Any:
    if payload is None:
        return None
    if isinstance(payload, Enum):
        return payload.value
    if is_dataclass(payload):
        return {key: encode_payload(value) for key, value in asdict(payload).items()}
    if isinstance(payload, dict):
        return {key: encode_payload(value) for key, value in payload.items()}
    if isinstance(payload, (list, tuple)):
        return [encode_payload(item) for item in payload]
    return payload


@dataclass(slots=True)
class ErrorResponse:
    error: str


@dataclass(slots=True)
class HealthResponse:
    status: str
    version: str


@dataclass(slots=True)
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
    target_name: str | None = None
    target_hp_pct: float | None = None
    pet_name: str | None = None


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


@dataclass(slots=True)
class EconomyStatusResponse:
    active_cycles: list[str]
    is_paused: bool


@dataclass(slots=True)
class EconomyLedgerResponse:
    plat_per_hour: float
    items_distributed: int
    vendor_sales: int


@dataclass(slots=True)
class EconomyQueuesResponse:
    loot_queue_len: int
    vendor_backlog_len: int


class AccountStatus(str, Enum):
    ACTIVE = "active"
    LOCKED = "locked"
    BANNED = "banned"


@dataclass(slots=True)
class AccountRecord:
    id: str
    name: str
    server: str
    character: str
    class_name: str = ""
    group: int = 0
    status: AccountStatus = AccountStatus.ACTIVE
    has_password: bool = False

    @classmethod
    def from_api(cls, payload: dict[str, Any]) -> "AccountRecord":
        normalized = dict(payload)
        if "class" in normalized:
            normalized["class_name"] = normalized.pop("class")
        return model_from_dict(cls, normalized)

    def to_api(self) -> dict[str, Any]:
        payload = encode_payload(self)
        payload["class"] = payload.pop("class_name")
        return payload


@dataclass(slots=True)
class SoulState:
    character_id: str
    mood: str
    personality_traits: list[str]
    memory_count: int
    last_event: str | None = None


@dataclass(slots=True)
class AuditEntry:
    id: int
    character_id: int
    action_type: str
    action_json: dict[str, JSONValue]
    reason: str | None
    operator_id: str | None
    created_at: str


@dataclass(slots=True)
class AuditPage:
    total: int
    offset: int
    limit: int
    entries: list[AuditEntry]


@dataclass(slots=True)
class AlertsResponse:
    alerts: list[dict[str, JSONValue]]
    unread_count: int


@dataclass(slots=True)
class AckResponse:
    updated: int
    unread_count: int


@dataclass(slots=True)
class CsvExport:
    text: str
    content_type: str
    filename: str | None = None
