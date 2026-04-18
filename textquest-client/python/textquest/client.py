import httpx
from typing import Optional, List, Dict, Any
from dataclasses import dataclass

from .types import (
    SessionInfo,
    HealthResponse,
    ErrorResponse,
    CommandResponse,
    GroupAssignment,
    TextQuestError,
)


@dataclass
class ClientConfig:
    base_url: str = "http://localhost:3001"
    api_token: Optional[str] = None
    timeout_secs: float = 30.0


class Client:
    def __init__(self, config: Optional[ClientConfig] = None):
        self.config = config or ClientConfig()
        self._client = httpx.Client(
            base_url=self.config.base_url,
            timeout=self.config.timeout_secs,
            headers={"Content-Type": "application/json"},
        )
        if self.config.api_token:
            self._client.headers["X-API-Token"] = self.config.api_token

    def _request(self, method: str, path: str, **kwargs) -> Any:
        url = f"{self.config.base_url}{path}"
        try:
            response = self._client.request(method, url, **kwargs)
            response.raise_for_status()
            if response.status_code == 204:
                return None
            return response.json()
        except httpx.HTTPStatusError as e:
            error_data = e.response.json() if e.response.content else {"error": "Unknown error"}
            raise TextQuestError(
                error_data.get("error", str(e)),
                status_code=e.response.status_code,
                response=error_data,
            )
        except httpx.RequestError as e:
            raise TextQuestError(str(e))

    def health(self) -> HealthResponse:
        data = self._request("GET", "/api/health")
        return HealthResponse(**data)

    def list_sessions(self) -> List[SessionInfo]:
        data = self._request("GET", "/api/sessions")
        return [SessionInfo(**session) for session in data]

    def pause_session(self, session_id: int) -> None:
        self._request("PUT", f"/api/control/pause/{session_id}")

    def resume_session(self, session_id: int) -> None:
        self._request("PUT", f"/api/control/resume/{session_id}")

    def set_session_group(self, session_id: int, group_id: int) -> None:
        assignment = GroupAssignment(group_id=group_id)
        self._request("PUT", f"/api/control/group/{session_id}", json=assignment.__dict__)

    def broadcast_all(self, session_id: int) -> None:
        self._request("PUT", f"/api/control/broadcast-all/{session_id}")

    def relay_command(self, command: str, target: Optional[str] = None) -> str:
        body = {"command": command}
        if target:
            body["target"] = target
        response = self._request("POST", "/api/command", json=body)
        return response["message"]

    def close(self) -> None:
        self._client.close()

    def __enter__(self) -> "Client":
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.close()


class AsyncClient:
    def __init__(self, config: Optional[ClientConfig] = None):
        self.config = config or ClientConfig()
        self._client = httpx.AsyncClient(
            base_url=self.config.base_url,
            timeout=self.config.timeout_secs,
            headers={"Content-Type": "application/json"},
        )
        if self.config.api_token:
            self._client.headers["X-API-Token"] = self.config.api_token

    async def _request(self, method: str, path: str, **kwargs) -> Any:
        url = f"{self.config.base_url}{path}"
        try:
            response = await self._client.request(method, url, **kwargs)
            response.raise_for_status()
            if response.status_code == 204:
                return None
            return response.json()
        except httpx.HTTPStatusError as e:
            error_data = e.response.json() if e.response.content else {"error": "Unknown error"}
            raise TextQuestError(
                error_data.get("error", str(e)),
                status_code=e.response.status_code,
                response=error_data,
            )
        except httpx.RequestError as e:
            raise TextQuestError(str(e))

    async def health(self) -> HealthResponse:
        data = await self._request("GET", "/api/health")
        return HealthResponse(**data)

    async def list_sessions(self) -> List[SessionInfo]:
        data = await self._request("GET", "/api/sessions")
        return [SessionInfo(**session) for session in data]

    async def pause_session(self, session_id: int) -> None:
        await self._request("PUT", f"/api/control/pause/{session_id}")

    async def resume_session(self, session_id: int) -> None:
        await self._request("PUT", f"/api/control/resume/{session_id}")

    async def set_session_group(self, session_id: int, group_id: int) -> None:
        assignment = GroupAssignment(group_id=group_id)
        await self._request("PUT", f"/api/control/group/{session_id}", json=assignment.__dict__)

    async def broadcast_all(self, session_id: int) -> None:
        await self._request("PUT", f"/api/control/broadcast-all/{session_id}")

    async def relay_command(self, command: str, target: Optional[str] = None) -> str:
        body = {"command": command}
        if target:
            body["target"] = target
        response = await self._request("POST", "/api/command", json=body)
        return response["message"]

    async def close(self) -> None:
        await self._client.aclose()

    async def __aenter__(self) -> "AsyncClient":
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:
        await self.close()