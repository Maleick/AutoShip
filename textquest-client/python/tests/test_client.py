from __future__ import annotations

import asyncio
import json
import threading

import pytest
from aiohttp import WSMsgType, web

from textquest import (
    AsyncTextQuestClient,
    NotFoundError,
    TextQuestClient,
    TextQuestNotImplementedError,
    TextQuestTimeoutError,
    UnauthorizedError,
)


@pytest.fixture
def mock_api_server() -> tuple[str, dict[str, object]]:
    state: dict[str, object] = {
        "token": "secret-token",
        "retry_attempts": 0,
        "ws_connections": 0,
    }

    async def require_auth(request: web.Request) -> web.StreamResponse | None:
        expected = str(state["token"])
        provided = request.headers.get("X-API-Token", "")
        if provided != expected:
            return web.json_response(
                {"error": "missing or invalid api token"},
                status=401,
            )
        return None

    async def health(request: web.Request) -> web.StreamResponse:
        unauthorized = await require_auth(request)
        if unauthorized is not None:
            return unauthorized
        return web.json_response({"status": "ok", "version": "1.2.3"})

    async def sessions(request: web.Request) -> web.StreamResponse:
        unauthorized = await require_auth(request)
        if unauthorized is not None:
            return unauthorized
        return web.json_response(
            [
                {
                    "client_id": 7,
                    "character_name": "Frostreaver",
                    "zone": "Lower Guk",
                    "level": 60,
                    "hp_pct": 98.5,
                    "mana_pct": 87.0,
                    "endurance_pct": 70.5,
                    "status": "idle",
                    "buff_count": 12,
                    "target_name": "Ghoul Lord",
                    "target_hp_pct": 66.0,
                    "pet_name": None,
                }
            ]
        )

    async def soul_audit_csv(request: web.Request) -> web.StreamResponse:
        unauthorized = await require_auth(request)
        if unauthorized is not None:
            return unauthorized
        csv_body = "id,character_id,action_type\n1,1,say\n"
        response = web.Response(text=csv_body, content_type="text/csv")
        response.headers["Content-Disposition"] = (
            'attachment; filename="soul_audit_1.csv"'
        )
        return response

    async def not_found(request: web.Request) -> web.StreamResponse:
        unauthorized = await require_auth(request)
        if unauthorized is not None:
            return unauthorized
        return web.json_response({"error": "API route not found"}, status=404)

    async def not_implemented(request: web.Request) -> web.StreamResponse:
        unauthorized = await require_auth(request)
        if unauthorized is not None:
            return unauthorized
        return web.json_response(
            {"error": "Raid configuration API is not implemented in this build"},
            status=501,
        )

    async def retry_timeout(request: web.Request) -> web.StreamResponse:
        unauthorized = await require_auth(request)
        if unauthorized is not None:
            return unauthorized
        state["retry_attempts"] = int(state["retry_attempts"]) + 1
        if state["retry_attempts"] == 1:
            await asyncio.sleep(0.15)
        return web.json_response({"status": "recovered", "version": "retry"})

    async def ws_handler(request: web.Request) -> web.StreamResponse:
        if request.query.get("token") != state["token"]:
            return web.Response(status=401)
        websocket = web.WebSocketResponse()
        await websocket.prepare(request)
        state["ws_connections"] = int(state["ws_connections"]) + 1
        await websocket.send_str("session:update")
        await websocket.send_str(
            json.dumps(
                {
                    "type": "dashboard_snapshot",
                    "source": "dashboard",
                    "snapshot": {"generatedAt": "2026-04-17T12:00:00Z"},
                }
            )
        )
        async for message in websocket:
            if message.type == WSMsgType.PING:
                await websocket.pong(message.data)
        return websocket

    app = web.Application()
    app.router.add_get("/api/health", health)
    app.router.add_get("/api/sessions", sessions)
    app.router.add_get("/api/soul/audit/1/export.csv", soul_audit_csv)
    app.router.add_get("/api/not-real", not_found)
    app.router.add_get("/api/raid/config", not_implemented)
    app.router.add_get("/api/retry-timeout", retry_timeout)
    app.router.add_get("/ws", ws_handler)

    loop = asyncio.new_event_loop()
    ready = threading.Event()
    holder: dict[str, object] = {}

    def run_server() -> None:
        asyncio.set_event_loop(loop)
        runner = web.AppRunner(app)

        async def start() -> None:
            await runner.setup()
            site = web.TCPSite(runner, "127.0.0.1", 0)
            await site.start()
            sockets = site._server.sockets
            assert sockets
            holder["runner"] = runner
            holder["port"] = sockets[0].getsockname()[1]
            ready.set()

        loop.run_until_complete(start())
        loop.run_forever()
        loop.run_until_complete(runner.cleanup())
        loop.close()

    thread = threading.Thread(target=run_server, daemon=True)
    thread.start()
    if not ready.wait(timeout=5):
        raise RuntimeError("Mock API server failed to start within 5 seconds")

    yield f"http://127.0.0.1:{holder['port']}", state

    loop.call_soon_threadsafe(loop.stop)
    thread.join(timeout=5)


def test_sync_client_reads_typed_models_and_auth(mock_api_server: tuple[str, dict[str, object]]) -> None:
    base_url, _state = mock_api_server

    client = TextQuestClient(base_url=base_url, api_token="secret-token")
    health = client.health()
    sessions = client.list_sessions()
    client.close()

    assert health.status == "ok"
    assert health.version == "1.2.3"
    assert len(sessions) == 1
    assert sessions[0].character_name == "Frostreaver"
    assert sessions[0].target_name == "Ghoul Lord"


def test_sync_client_raises_typed_http_errors(mock_api_server: tuple[str, dict[str, object]]) -> None:
    base_url, _state = mock_api_server

    unauthorized_client = TextQuestClient(base_url=base_url, api_token="wrong-token")
    with pytest.raises(UnauthorizedError):
        unauthorized_client.health()
    unauthorized_client.close()

    client = TextQuestClient(base_url=base_url, api_token="secret-token")
    with pytest.raises(NotFoundError):
        client.get_json("/api/not-real")
    with pytest.raises(TextQuestNotImplementedError):
        client.get_json("/api/raid/config")
    client.close()


def test_sync_client_retries_timeouts(mock_api_server: tuple[str, dict[str, object]]) -> None:
    base_url, state = mock_api_server

    client = TextQuestClient(
        base_url=base_url,
        api_token="secret-token",
        timeout=0.05,
        max_retries=1,
        retry_backoff=0.0,
    )
    payload = client.get_json("/api/retry-timeout")
    client.close()

    assert payload["status"] == "recovered"
    assert state["retry_attempts"] == 2


def test_sync_client_exposes_csv_export_metadata(mock_api_server: tuple[str, dict[str, object]]) -> None:
    base_url, _state = mock_api_server

    client = TextQuestClient(base_url=base_url, api_token="secret-token")
    export = client.export_soul_audit_csv(character_id=1)
    client.close()

    assert export.filename == "soul_audit_1.csv"
    assert export.content_type == "text/csv"
    assert "character_id" in export.text


@pytest.mark.asyncio
async def test_async_client_timeout_error_when_retries_exhausted(
    mock_api_server: tuple[str, dict[str, object]],
) -> None:
    base_url, _state = mock_api_server

    client = AsyncTextQuestClient(
        base_url=base_url,
        api_token="secret-token",
        timeout=0.01,
        max_retries=0,
    )
    with pytest.raises(TextQuestTimeoutError):
        await client.get_json("/api/retry-timeout")
    await client.aclose()


@pytest.mark.asyncio
async def test_websocket_helper_streams_text_and_json_events(
    mock_api_server: tuple[str, dict[str, object]],
) -> None:
    base_url, state = mock_api_server

    async with AsyncTextQuestClient(
        base_url=base_url,
        api_token="secret-token",
    ) as client:
        async with client.websocket() as websocket:
            first = await websocket.recv()
            second = await websocket.recv()

    assert first.raw == "session:update"
    assert first.json is None
    assert second.event_type == "dashboard_snapshot"
    assert second.json is not None
    assert second.json["source"] == "dashboard"
    assert state["ws_connections"] == 1
