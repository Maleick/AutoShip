from __future__ import annotations

import asyncio
import json
from dataclasses import dataclass
from typing import Any, Optional, Callable, Awaitable, Dict
from urllib.parse import ParseResult, urlencode, urlparse, urlunparse

import websockets

from .types import SessionEvent, SessionEventType


def build_websocket_url(base_url: str, api_token: str | None = None) -> str:
    parsed = urlparse(base_url)
    scheme = "wss" if parsed.scheme == "https" else "ws"
    path = "/ws"
    query = urlencode({"token": api_token}) if api_token else ""
    return urlunparse(
        ParseResult(
            scheme=scheme,
            netloc=parsed.netloc,
            path=path,
            params="",
            query=query,
            fragment="",
        )
    )


@dataclass(slots=True)
class WebSocketEvent:
    raw: str
    json: dict[str, Any] | list[Any] | None = None
    event_type: str | None = None

    @classmethod
    def from_message(cls, message: str) -> "WebSocketEvent":
        try:
            parsed = json.loads(message)
        except json.JSONDecodeError:
            return cls(raw=message)

        event_type = parsed.get("type") if isinstance(parsed, dict) else None
        if isinstance(parsed, (dict, list)):
            return cls(raw=message, json=parsed, event_type=event_type)
        return cls(raw=message)


class TextQuestWebSocket:
    def __init__(self, url: str) -> None:
        self._url = url
        self._socket: Any = None

    async def __aenter__(self) -> "TextQuestWebSocket":
        self._socket = await websockets.connect(self._url)
        return self

    async def __aexit__(self, exc_type: Any, exc: Any, tb: Any) -> None:
        if self._socket is not None:
            await self._socket.close()
            self._socket = None

    async def recv(self) -> WebSocketEvent:
        if self._socket is None:
            raise RuntimeError("WebSocket connection is not open")
        message = await self._socket.recv()
        if not isinstance(message, str):
            raise TypeError("Expected text WebSocket frames from TextQuest")
        return WebSocketEvent.from_message(message)


class WebSocketClient:
    """Legacy callback-based WebSocket client. Prefer TextQuestWebSocket for new code."""

    def __init__(
        self,
        base_url: str = "ws://localhost:3001",
        api_token: Optional[str] = None,
    ):
        self.base_url = base_url.replace("http", "ws")
        self.api_token = api_token
        self._ws: Any = None
        self._event_callback: Optional[Callable[[SessionEvent], Awaitable[None]]] = None

    async def connect(
        self,
        on_event: Callable[[SessionEvent], Awaitable[None]],
    ) -> None:
        self._event_callback = on_event

        url = self.base_url
        if self.api_token:
            url = f"{url}?token={self.api_token}"

        self._ws = await websockets.connect(url)
        asyncio.create_task(self._receive_loop())

    async def _receive_loop(self) -> None:
        if not self._ws:
            return

        try:
            async for message in self._ws:
                try:
                    data = json.loads(message)
                    event = self._parse_event(data)
                    if self._event_callback:
                        await self._event_callback(event)
                except json.JSONDecodeError:
                    pass
        except websockets.ConnectionClosed:
            pass
        finally:
            self._ws = None

    def _parse_event(self, data: Dict[str, Any]) -> SessionEvent:
        event_type = data.get("type", "unknown")
        session_type = SessionEventType(event_type)
        return SessionEvent(type=session_type, data=data)

    async def send_command(self, command: str, target: Optional[str] = None) -> None:
        if not self._ws:
            raise RuntimeError("WebSocket not connected")

        message: dict[str, Any] = {"type": "command", "command": command}
        if target:
            message["target"] = target

        await self._ws.send(json.dumps(message))

    async def disconnect(self) -> None:
        if self._ws:
            await self._ws.close()
            self._ws = None
