"""TextQuest Python Client Library"""

from .client import Client, AsyncClient, ClientConfig
from .websocket import WebSocketClient
from .types import (
    SessionInfo,
    CharacterConfig,
    HealthResponse,
    ErrorResponse,
    CommandRequest,
    CommandResponse,
    GroupAssignment,
    TextQuestError,
    SessionEvent,
)

__version__ = "0.1.0"

__all__ = [
    "Client",
    "AsyncClient", 
    "ClientConfig",
    "WebSocketClient",
    "SessionInfo",
    "CharacterConfig",
    "HealthResponse",
    "ErrorResponse",
    "CommandRequest",
    "CommandResponse",
    "GroupAssignment",
    "TextQuestError",
    "SessionEvent",
]