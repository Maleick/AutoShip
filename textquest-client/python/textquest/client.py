from __future__ import annotations

import asyncio
import time
from dataclasses import dataclass
from typing import Any, Optional, List
from urllib.parse import urljoin

import httpx

from .types import (
    AckResponse,
    AccountRecord,
    AlertsResponse,
    AuditPage,
    CommandResponse,
    CsvExport,
    EconomyLedgerResponse,
    EconomyQueuesResponse,
    EconomyStatusResponse,
    ErrorResponse,
    GroupAssignment,
    HealthResponse,
    SessionInfo,
    SoulState,
    encode_payload,
    model_from_dict,
)
from .websocket import TextQuestWebSocket, build_websocket_url


class TextQuestError(Exception):
    pass


class TextQuestTimeoutError(TextQuestError):
    pass


class TextQuestHttpError(TextQuestError):
    def __init__(self, message: str, *, status_code: int, response: httpx.Response) -> None:
        super().__init__(message)
        self.status_code = status_code
        self.response = response


class UnauthorizedError(TextQuestHttpError):
    pass


class NotFoundError(TextQuestHttpError):
    pass


class TextQuestNotImplementedError(TextQuestHttpError):
    pass


@dataclass(slots=True)
class _ClientConfig:
    base_url: str
    api_token: str | None
    timeout: float
    max_retries: int
    retry_backoff: float

    def headers(self) -> dict[str, str]:
        headers = {"Accept": "application/json"}
        if self.api_token:
            headers["Authorization"] = f"Bearer {self.api_token}"
        return headers


def _join_url(base_url: str, path: str) -> str:
    normalized = base_url if base_url.endswith("/") else f"{base_url}/"
    return urljoin(normalized, path.lstrip("/"))


def _extract_error_message(response: httpx.Response) -> str:
    try:
        payload = response.json()
    except ValueError:
        return response.text or f"HTTP {response.status_code}"

    if isinstance(payload, dict) and "error" in payload:
        return model_from_dict(ErrorResponse, payload).error
    return response.text or f"HTTP {response.status_code}"


def _raise_for_response(response: httpx.Response) -> None:
    if response.status_code < 400:
        return

    message = _extract_error_message(response)
    error_type: type[TextQuestHttpError]
    if response.status_code == 401:
        error_type = UnauthorizedError
    elif response.status_code == 404:
        error_type = NotFoundError
    elif response.status_code == 501:
        error_type = TextQuestNotImplementedError
    else:
        error_type = TextQuestHttpError
    raise error_type(message, status_code=response.status_code, response=response)


class TextQuestClient:
    def __init__(
        self,
        *,
        base_url: str,
        api_token: str | None = None,
        timeout: float = 5.0,
        max_retries: int = 0,
        retry_backoff: float = 0.25,
        transport: httpx.BaseTransport | None = None,
    ) -> None:
        self._config = _ClientConfig(
            base_url=base_url,
            api_token=api_token,
            timeout=timeout,
            max_retries=max_retries,
            retry_backoff=retry_backoff,
        )
        self._client = httpx.Client(
            headers=self._config.headers(),
            timeout=timeout,
            transport=transport,
        )

    def close(self) -> None:
        self._client.close()

    def __enter__(self) -> "TextQuestClient":
        return self

    def __exit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        self.close()

    def request(
        self,
        method: str,
        path: str,
        *,
        params: dict[str, Any] | None = None,
        json: Any = None,
    ) -> httpx.Response:
        url = _join_url(self._config.base_url, path)
        last_error: Exception | None = None
        for attempt in range(self._config.max_retries + 1):
            try:
                response = self._client.request(
                    method,
                    url,
                    params=params,
                    json=encode_payload(json),
                )
                _raise_for_response(response)
                return response
            except httpx.TimeoutException as error:
                last_error = error
                if attempt >= self._config.max_retries:
                    raise TextQuestTimeoutError(str(error)) from error
                if self._config.retry_backoff:
                    time.sleep(self._config.retry_backoff)
            except httpx.TransportError as error:
                last_error = error
                if attempt >= self._config.max_retries:
                    raise TextQuestError(str(error)) from error
                if self._config.retry_backoff:
                    time.sleep(self._config.retry_backoff)

        raise TextQuestError(str(last_error))

    def get_json(
        self,
        path: str,
        *,
        params: dict[str, Any] | None = None,
    ) -> dict[str, Any] | list[Any]:
        return self.request("GET", path, params=params).json()

    def health(self) -> HealthResponse:
        payload = self.get_json("/api/health")
        return model_from_dict(HealthResponse, payload)

    def list_sessions(self) -> list[SessionInfo]:
        payload = self.get_json("/api/sessions")
        return [model_from_dict(SessionInfo, item) for item in payload]

    def pause_session(self, session_id: int) -> None:
        self.request("PUT", f"/api/control/pause/{session_id}")

    def resume_session(self, session_id: int) -> None:
        self.request("PUT", f"/api/control/resume/{session_id}")

    def set_session_group(self, session_id: int, group_id: int) -> None:
        assignment = GroupAssignment(group_id=group_id)
        self.request("PUT", f"/api/control/group/{session_id}", json=assignment)

    def broadcast_all(self, session_id: int) -> None:
        self.request("PUT", f"/api/control/broadcast-all/{session_id}")

    def relay_command(self, command: str, target: Optional[str] = None) -> str:
        body: dict[str, Any] = {"command": command}
        if target:
            body["target"] = target
        response = self.request("POST", "/api/command", json=body)
        return model_from_dict(CommandResponse, response.json()).message

    def get_economy_status(self) -> EconomyStatusResponse:
        payload = self.get_json("/api/economy/status")
        return model_from_dict(EconomyStatusResponse, payload)

    def get_economy_ledger(self) -> EconomyLedgerResponse:
        payload = self.get_json("/api/economy/ledger")
        return model_from_dict(EconomyLedgerResponse, payload)

    def get_economy_queues(self) -> EconomyQueuesResponse:
        payload = self.get_json("/api/economy/queues")
        return model_from_dict(EconomyQueuesResponse, payload)

    def list_accounts(self) -> list[AccountRecord]:
        payload = self.get_json("/api/accounts")
        return [AccountRecord.from_api(item) for item in payload]

    def list_soul_states(self) -> list[SoulState]:
        payload = self.get_json("/api/soul")
        return [model_from_dict(SoulState, item) for item in payload]

    def get_soul_state(self, character_id: str) -> SoulState:
        payload = self.get_json(f"/api/soul/{character_id}")
        return model_from_dict(SoulState, payload)

    def get_soul_audit(
        self,
        *,
        character_id: int | None = None,
        offset: int = 0,
        limit: int | None = None,
        start: str | None = None,
        end: str | None = None,
    ) -> AuditPage:
        params: dict[str, Any] = {"offset": offset}
        if limit is not None:
            params["limit"] = limit
        if start is not None:
            params["start"] = start
        if end is not None:
            params["end"] = end
        path = "/api/soul/audit"
        if character_id is not None:
            path = f"/api/soul/audit/{character_id}"
        payload = self.get_json(path, params=params)
        return model_from_dict(AuditPage, payload)

    def export_soul_audit_csv(self, *, character_id: int | None = None) -> CsvExport:
        path = "/api/soul/audit/export.csv"
        if character_id is not None:
            path = f"/api/soul/audit/{character_id}/export.csv"
        response = self.request("GET", path)
        content_type = response.headers.get("Content-Type", "").split(";")[0]
        disposition = response.headers.get("Content-Disposition", "")
        filename: str | None = None
        if "filename=" in disposition:
            filename = disposition.split("filename=", 1)[1].strip().strip('"')
        return CsvExport(
            text=response.text,
            content_type=content_type,
            filename=filename,
        )

    def list_alerts(self) -> AlertsResponse:
        payload = self.get_json("/api/alerts")
        return model_from_dict(AlertsResponse, payload)

    def ack_alert(self, alert_id: int) -> AckResponse:
        response = self.request("POST", f"/api/alerts/{alert_id}/ack")
        return model_from_dict(AckResponse, response.json())

    def ack_all_alerts(self) -> AckResponse:
        response = self.request("POST", "/api/alerts/ack-all")
        return model_from_dict(AckResponse, response.json())


class AsyncTextQuestClient:
    def __init__(
        self,
        *,
        base_url: str,
        api_token: str | None = None,
        timeout: float = 5.0,
        max_retries: int = 0,
        retry_backoff: float = 0.25,
        transport: httpx.AsyncBaseTransport | None = None,
    ) -> None:
        self._config = _ClientConfig(
            base_url=base_url,
            api_token=api_token,
            timeout=timeout,
            max_retries=max_retries,
            retry_backoff=retry_backoff,
        )
        self._client = httpx.AsyncClient(
            headers=self._config.headers(),
            timeout=timeout,
            transport=transport,
        )

    async def __aenter__(self) -> "AsyncTextQuestClient":
        return self

    async def __aexit__(self, exc_type: Any, exc: Any, tb: Any) -> None:
        await self.aclose()

    async def aclose(self) -> None:
        await self._client.aclose()

    async def request(
        self,
        method: str,
        path: str,
        *,
        params: dict[str, Any] | None = None,
        json: Any = None,
    ) -> httpx.Response:
        url = _join_url(self._config.base_url, path)
        last_error: Exception | None = None
        for attempt in range(self._config.max_retries + 1):
            try:
                response = await self._client.request(
                    method,
                    url,
                    params=params,
                    json=encode_payload(json),
                )
                _raise_for_response(response)
                return response
            except httpx.TimeoutException as error:
                last_error = error
                if attempt >= self._config.max_retries:
                    raise TextQuestTimeoutError(str(error)) from error
                if self._config.retry_backoff:
                    await asyncio.sleep(self._config.retry_backoff)
            except httpx.TransportError as error:
                last_error = error
                if attempt >= self._config.max_retries:
                    raise TextQuestError(str(error)) from error
                if self._config.retry_backoff:
                    await asyncio.sleep(self._config.retry_backoff)

        raise TextQuestError(str(last_error))

    async def get_json(
        self,
        path: str,
        *,
        params: dict[str, Any] | None = None,
    ) -> dict[str, Any] | list[Any]:
        response = await self.request("GET", path, params=params)
        return response.json()

    async def health(self) -> HealthResponse:
        payload = await self.get_json("/api/health")
        return model_from_dict(HealthResponse, payload)

    async def list_sessions(self) -> list[SessionInfo]:
        payload = await self.get_json("/api/sessions")
        return [model_from_dict(SessionInfo, item) for item in payload]

    async def pause_session(self, session_id: int) -> None:
        await self.request("PUT", f"/api/control/pause/{session_id}")

    async def resume_session(self, session_id: int) -> None:
        await self.request("PUT", f"/api/control/resume/{session_id}")

    async def relay_command(self, command: str, target: Optional[str] = None) -> str:
        body: dict[str, Any] = {"command": command}
        if target:
            body["target"] = target
        response = await self.request("POST", "/api/command", json=body)
        return model_from_dict(CommandResponse, response.json()).message

    def websocket(self) -> TextQuestWebSocket:
        return TextQuestWebSocket(
            build_websocket_url(self._config.base_url, self._config.api_token)
        )
