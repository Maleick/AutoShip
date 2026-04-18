import asyncio
import json
from typing import Optional, Callable, Awaitable, Dict, Any
from enum import Enum

import websockets

from .types import TextQuestError, SessionEvent, SessionEventType


class WebSocketClient:
    def __init__(
        self,
        base_url: str = "ws://localhost:3001",
        api_token: Optional[str] = None,
    ):
        self.base_url = base_url.replace("http", "ws")
        self.api_token = api_token
        self._ws: Optional[websockets.WebSocketClientProtocol] = None
        self._event_callback: Optional[Callable[[SessionEvent], Awaitable[None]]] = None

    async def connect(
        self,
        on_event: Callable[[SessionEvent], Awaitable[None]],
    ) -> None:
        self._event_callback = on_event

        url = self.base_url
        if self.api_token:
            url = f"{url}?token={self.api_token}"

        try:
            self._ws = await websockets.connect(url)
            asyncio.create_task(self._receive_loop())
        except Exception as e:
            raise TextQuestError(f"WebSocket connection failed: {e}")

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
            raise TextQuestError("WebSocket not connected")

        message = {"type": "command", "command": command}
        if target:
            message["target"] = target

        await self._ws.send(json.dumps(message))

    async def disconnect(self) -> None:
        if self._ws:
            await self._ws.close()
            self._ws = None