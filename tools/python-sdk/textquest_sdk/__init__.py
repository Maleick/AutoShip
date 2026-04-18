"""TextQuest Python SDK for the TextQuest Web API.

This package provides a clean, type-hinted Python interface for interacting with
the TextQuest web API. It supports both sync and async usage patterns.

Example (sync):
    >>> from textquest_sdk import TextQuestClient
    >>> client = TextQuestClient("http://localhost:3001")
    >>> health = client.health()
    >>> print(health)

Example (async):
    >>> import asyncio
    >>> from textquest_sdk import AsyncTextQuestClient
    >>> async def main():
    ...     async with AsyncTextQuestClient("http://localhost:3001") as client:
    ...         health = await client.health()
    ...         print(health)
    >>> asyncio.run(main())
"""

__version__ = "0.1.0"

from textquest_sdk.client import TextQuestClient
from textquest_sdk.async_client import AsyncTextQuestClient
from textquest_sdk.models import (
    HealthResponse,
    ErrorResponse,
    SessionInfo,
    Account,
    CharacterConfig,
    EconomySettings,
    VendorRoute,
    WealthSnapshot,
    LootRules,
    LootFilters,
    SoulState,
    AlertConfig,
)

__all__ = [
    "TextQuestClient",
    "AsyncTextQuestClient",
    "HealthResponse",
    "ErrorResponse",
    "SessionInfo",
    "Account",
    "CharacterConfig",
    "EconomySettings",
    "VendorRoute",
    "WealthSnapshot",
    "LootRules",
    "LootFilters",
    "SoulState",
    "AlertConfig",
]
