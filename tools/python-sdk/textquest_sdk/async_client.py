"""Asynchronous TextQuest API client.

This module provides an async wrapper around the TextQuest web API using httpx.
"""

from typing import Any, Dict, List, Optional
from urllib.parse import quote
import httpx

from textquest_sdk.models import (
    HealthResponse,
    SessionInfo,
    Account,
    CharacterConfig,
    EconomySettings,
    VendorRoute,
    WealthSnapshot,
    LootRules,
    AlertConfig,
)


class AsyncTextQuestClient:
    """Asynchronous client for the TextQuest Web API.

    Example:
        >>> async with AsyncTextQuestClient("http://localhost:3001") as client:
        ...     health = await client.health()
        ...     sessions = await client.list_sessions()
    """

    def __init__(self, base_url: str = "http://localhost:3001", api_token: Optional[str] = None):
        """Initialize the async TextQuest client.

        Args:
            base_url: The base URL of the TextQuest web API (default: http://localhost:3001)
            api_token: Optional API token for authentication (X-API-Token header)
        """
        self.base_url = base_url.rstrip("/")
        self.api_token = api_token
        self.client: Optional[httpx.AsyncClient] = None

    async def _ensure_client(self) -> httpx.AsyncClient:
        """Ensure the httpx client is initialized."""
        if self.client is None:
            headers = {}
            if self.api_token:
                headers["X-API-Token"] = self.api_token
            self.client = httpx.AsyncClient(base_url=self.base_url, headers=headers, timeout=10)
        return self.client

    async def _make_request(
        self,
        method: str,
        endpoint: str,
        json_data: Optional[Dict[str, Any]] = None,
        params: Optional[Dict[str, Any]] = None,
    ) -> Any:
        """Make an async HTTP request to the API.

        Args:
            method: HTTP method (GET, POST, PUT, DELETE)
            endpoint: API endpoint path
            json_data: JSON body data
            params: Query parameters

        Returns:
            Parsed JSON response

        Raises:
            httpx.HTTPError: On network or HTTP errors
        """
        client = await self._ensure_client()
        response = await client.request(method, endpoint, json=json_data, params=params)
        response.raise_for_status()

        if response.text:
            return response.json()
        return None

    @staticmethod
    def _summarize_kill_tracker_history(history: List[Dict[str, Any]]) -> Dict[str, Any]:
        """Build a lightweight stats view from kill-tracker history."""
        total_kills = 0
        kills_by_zone: Dict[str, int] = {}
        total_sessions = 0

        for character_history in history:
            for session in character_history.get("sessions", []):
                total_sessions += 1
                kills = int(session.get("total_kills", 0))
                total_kills += kills
                zone = session.get("zone", "Unknown")
                kills_by_zone[zone] = kills_by_zone.get(zone, 0) + kills

        return {
            "total_kills": total_kills,
            "kills_by_zone": kills_by_zone,
            "total_loot_value": 0,
            "average_kill_value": 0.0,
            "session_count": total_sessions,
        }

    # ─── Health & Status ─────────────────────────────────────────────────

    async def health(self) -> Dict[str, str]:
        """Get API health status.

        Returns:
            Health status with version information
        """
        return await self._make_request("GET", "/api/health")

    async def list_sessions(self) -> List[Dict[str, Any]]:
        """List all active sessions.

        Returns:
            List of session information
        """
        return await self._make_request("GET", "/api/sessions")

    # ─── Accounts ───────────────────────────────────────────────────────

    async def create_account(self, account_data: Dict[str, Any]) -> Dict[str, Any]:
        """Create a new account.

        Args:
            account_data: Account information (name, server, character, class, group, status, password)

        Returns:
            Created account details
        """
        return await self._make_request("POST", "/api/accounts", json_data=account_data)

    async def list_accounts(self) -> List[Dict[str, Any]]:
        """List all accounts.

        Returns:
            List of account information
        """
        return await self._make_request("GET", "/api/accounts")

    async def get_account(self, name: str) -> Dict[str, Any]:
        """Get account details by name.

        Args:
            name: Account name

        Returns:
            Account information
        """
        return await self._make_request("GET", f"/api/accounts/{name}")

    async def update_account(self, name: str, account_data: Dict[str, Any]) -> Dict[str, Any]:
        """Update account information.

        Args:
            name: Account name
            account_data: Updated account fields

        Returns:
            Updated account details
        """
        return await self._make_request("PUT", f"/api/accounts/{name}", json_data=account_data)

    async def set_account_password(self, name: str, password: str) -> None:
        """Set or update account password.

        Args:
            name: Account name
            password: New password

        Raises:
            httpx.HTTPError: If credential store is not enabled
        """
        await self._make_request("PUT", f"/api/accounts/{name}/password", json_data={"password": password})

    async def delete_account(self, name: str) -> None:
        """Delete an account.

        Args:
            name: Account name
        """
        client = await self._ensure_client()
        response = await client.delete(f"/api/accounts/{name}")
        response.raise_for_status()

    async def export_accounts(self) -> Dict[str, Any]:
        """Export all accounts.

        Returns:
            Exported accounts data
        """
        return await self._make_request("GET", "/api/accounts/export")

    async def import_accounts(self, accounts_data: List[Dict[str, Any]]) -> Dict[str, Any]:
        """Import accounts.

        Args:
            accounts_data: List of account data to import

        Returns:
            Import result with count
        """
        return await self._make_request("POST", "/api/accounts/import", json_data={"accounts": accounts_data})

    # ─── Economy ────────────────────────────────────────────────────────

    async def get_economy_settings(self) -> Dict[str, Any]:
        """Get economy configuration.

        Returns:
            Economy settings
        """
        return await self._make_request("GET", "/api/economy/settings")

    async def set_economy_settings(self, settings: Dict[str, Any]) -> Dict[str, Any]:
        """Update economy configuration.

        Args:
            settings: Economy settings to update

        Returns:
            Updated economy settings
        """
        return await self._make_request("PUT", "/api/economy/settings", json_data=settings)

    async def list_vendor_routes(self) -> List[Dict[str, Any]]:
        """List all vendor routes.

        Returns:
            List of vendor routes
        """
        return await self._make_request("GET", "/api/economy/vendor-routes")

    async def create_vendor_route(self, route_data: Dict[str, Any]) -> Dict[str, Any]:
        """Create a new vendor route.

        Args:
            route_data: Route information (name, vendors, priority)

        Returns:
            Created route details
        """
        return await self._make_request("POST", "/api/economy/vendor-routes", json_data=route_data)

    async def update_vendor_route(self, route_id: str, route_data: Dict[str, Any]) -> Dict[str, Any]:
        """Update a vendor route.

        Args:
            route_id: Route ID
            route_data: Updated route fields

        Returns:
            Updated route details
        """
        return await self._make_request("PUT", f"/api/economy/vendor-routes/{route_id}", json_data=route_data)

    async def delete_vendor_route(self, route_id: str) -> None:
        """Delete a vendor route.

        Args:
            route_id: Route ID
        """
        client = await self._ensure_client()
        response = await client.delete(f"/api/economy/vendor-routes/{route_id}")
        response.raise_for_status()

    async def get_wealth(self) -> Dict[str, Any]:
        """Get current wealth snapshot.

        Returns:
            Wealth information for all characters
        """
        return await self._make_request("GET", "/api/economy/wealth")

    # ─── Soul States ────────────────────────────────────────────────────

    async def list_soul_states(self) -> List[Dict[str, Any]]:
        """List all character soul states.

        Returns:
            List of soul state information
        """
        return await self._make_request("GET", "/api/soul")

    async def get_soul_state(self, character_id: str) -> Dict[str, Any]:
        """Get soul state for a character.

        Args:
            character_id: Character ID

        Returns:
            Soul state information
        """
        return await self._make_request("GET", f"/api/soul/{character_id}")

    async def get_soul_audit(self) -> List[Dict[str, Any]]:
        """Get all soul audit entries.

        Returns:
            List of audit entries
        """
        return await self._make_request("GET", "/api/soul/audit")

    async def get_character_audit(self, character_id: str) -> List[Dict[str, Any]]:
        """Get soul audit entries for a character.

        Args:
            character_id: Character ID

        Returns:
            List of audit entries for the character
        """
        return await self._make_request("GET", f"/api/soul/audit/{character_id}")

    async def export_audit_csv(self) -> str:
        """Export all audit data as CSV.

        Returns:
            CSV content
        """
        client = await self._ensure_client()
        response = await client.get("/api/soul/audit/export.csv")
        response.raise_for_status()
        return response.text

    async def export_character_audit_csv(self, character_id: str) -> str:
        """Export character audit data as CSV.

        Args:
            character_id: Character ID

        Returns:
            CSV content
        """
        client = await self._ensure_client()
        response = await client.get(f"/api/soul/audit/{character_id}/export.csv")
        response.raise_for_status()
        return response.text

    # ─── Loot Management ────────────────────────────────────────────────

    async def get_loot_rules(self) -> Dict[str, Any]:
        """Get loot distribution rules.

        Returns:
            Loot rules configuration
        """
        return await self._make_request("GET", "/api/loot/rules")

    async def set_loot_rules(self, rules: Dict[str, Any]) -> Dict[str, Any]:
        """Update loot distribution rules.

        Args:
            rules: Loot rules configuration

        Returns:
            Updated loot rules
        """
        return await self._make_request("PUT", "/api/loot/rules", json_data=rules)

    async def get_loot_filters(self) -> List[Dict[str, Any]]:
        """Get all loot filters.

        Returns:
            List of loot filters
        """
        return await self._make_request("GET", "/api/loot/filters")

    async def set_loot_filter(self, character: str, filter_data: Dict[str, Any]) -> Dict[str, Any]:
        """Set loot filter for a character.

        Args:
            character: Character name
            filter_data: Filter configuration

        Returns:
            Updated filter
        """
        return await self._make_request("PUT", f"/api/loot/filters/{character}", json_data=filter_data)

    async def get_master_looter(self) -> Dict[str, Any]:
        """Get master looter configuration.

        Returns:
            Master looter information
        """
        return await self._make_request("GET", "/api/loot/master-looter")

    async def set_master_looter(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set master looter configuration.

        Args:
            config: Master looter configuration

        Returns:
            Updated configuration
        """
        return await self._make_request("PUT", "/api/loot/master-looter", json_data=config)

    async def get_loot_distribution(self) -> Dict[str, Any]:
        """Get loot distribution configuration.

        Returns:
            Distribution configuration
        """
        return await self._make_request("GET", "/api/loot/distribution")

    async def set_loot_distribution(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set loot distribution configuration.

        Args:
            config: Distribution configuration

        Returns:
            Updated configuration
        """
        return await self._make_request("PUT", "/api/loot/distribution", json_data=config)

    async def get_loot_history(self) -> List[Dict[str, Any]]:
        """Get loot distribution history.

        Returns:
            List of historical loot entries
        """
        return await self._make_request("GET", "/api/loot/history")

    # ─── Alerts ─────────────────────────────────────────────────────────

    async def get_alert_config(self) -> Dict[str, Any]:
        """Get alert configuration.

        Returns:
            Alert configuration
        """
        response = await self._make_request("GET", "/api/alerts/config")
        return response["config"]

    async def set_alert_config(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Update alert configuration.

        Args:
            config: Alert configuration

        Returns:
            Updated configuration
        """
        response = await self._make_request("PUT", "/api/alerts/config", json_data=config)
        return response["config"]

    async def list_alerts(self) -> Dict[str, Any]:
        """List all alerts.

        Returns:
            Alert envelope with alerts and unread count
        """
        return await self._make_request("GET", "/api/alerts")

    async def get_alert(self, alert_id: str) -> Dict[str, Any]:
        """Get specific alert.

        Args:
            alert_id: Alert ID

        Returns:
            Alert details
        """
        return await self._make_request("GET", f"/api/alerts/{alert_id}")

    # ─── Spawn Alerts ───────────────────────────────────────────────────

    async def list_spawn_alerts(self) -> List[Dict[str, Any]]:
        """List spawn alerts.

        Returns:
            List of spawn alerts
        """
        return await self._make_request("GET", "/api/spawn-alerts")

    async def clear_spawn_alerts(self) -> None:
        """Clear all spawn alerts."""
        client = await self._ensure_client()
        response = await client.delete("/api/spawn-alerts")
        response.raise_for_status()

    async def get_spawn_alert_stats(self) -> Dict[str, Any]:
        """Get spawn alert statistics.

        Returns:
            Spawn alert stats
        """
        return await self._make_request("GET", "/api/spawn-alerts/stats")

    async def get_spawn_alert_config(self) -> Dict[str, Any]:
        """Get spawn alert configuration.

        Returns:
            Spawn alert config
        """
        return await self._make_request("GET", "/api/spawn-alerts/config")

    async def set_spawn_alert_config(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set spawn alert configuration.

        Args:
            config: Configuration

        Returns:
            Updated config
        """
        return await self._make_request("PUT", "/api/spawn-alerts/config", json_data=config)

    async def get_spawn_watch_list(self) -> List[str]:
        """Get spawn watch list.

        Returns:
            List of watched spawn patterns
        """
        return await self._make_request("GET", "/api/spawn-alerts/watch-list")

    async def add_spawn_watch_pattern(self, pattern: str) -> Dict[str, Any]:
        """Add a spawn watch pattern.

        Args:
            pattern: Pattern to watch

        Returns:
            Updated watch list
        """
        return await self._make_request(
            "PUT",
            "/api/spawn-alerts/watch-list",
            json_data={"pattern": pattern},
        )

    async def remove_spawn_watch_pattern(self, pattern: str) -> None:
        """Remove a spawn watch pattern.

        Args:
            pattern: Pattern to remove
        """
        client = await self._ensure_client()
        response = await client.delete(f"/api/spawn-alerts/watch-list/{quote(pattern, safe='')}")
        response.raise_for_status()

    # ─── GM Alerts ───────────────────────────────────────────────────────

    async def list_gm_alerts(self) -> Dict[str, Any]:
        """Get the live GM alert status.

        Returns:
            GM alert status envelope
        """
        return await self.get_gm_alert_status()

    async def get_gm_alert_state(self) -> Dict[str, Any]:
        """Get current GM alert state.

        Returns:
            GM alert state
        """
        return await self.get_gm_alert_status()

    async def get_gm_alert_config(self) -> Dict[str, Any]:
        """Get the persisted GM alert configuration."""
        return await self._make_request("GET", "/api/gm-alerts/config")

    async def set_gm_alert_config(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Update the persisted GM alert configuration."""
        return await self._make_request("PUT", "/api/gm-alerts/config", json_data=config)

    async def get_gm_alert_status(self) -> Dict[str, Any]:
        """Get the live GM alert status."""
        return await self._make_request("GET", "/api/gm-alerts/status")

    async def sync_gm_alert_status(self, payload: Dict[str, Any]) -> Dict[str, Any]:
        """Push live GM alert status into the server-side cache."""
        return await self._make_request("POST", "/api/gm-alerts/sync", json_data=payload)

    async def acknowledge_gm_alert(self, alert_id: str) -> None:
        """Acknowledge a GM alert.

        Args:
            alert_id: Alert ID
        """
        client = await self._ensure_client()
        response = await client.put(f"/api/gm-alerts/{alert_id}/acknowledge")
        response.raise_for_status()

    # ─── Character Config ───────────────────────────────────────────────

    async def list_character_configs(self) -> List[Dict[str, Any]]:
        """List all character configurations.

        Returns:
            List of character configs
        """
        return await self._make_request("GET", "/api/config/characters")

    async def set_character_config(self, character: str, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set character configuration.

        Args:
            character: Character name
            config: Configuration data

        Returns:
            Updated config
        """
        return await self._make_request("PUT", f"/api/config/characters/{character}", json_data=config)

    async def get_auto_accept_settings(self) -> Dict[str, Any]:
        """Get auto-accept settings.

        Returns:
            Auto-accept configuration
        """
        return await self._make_request("GET", "/api/config/auto-accept")

    async def set_auto_accept_settings(self, settings: Dict[str, Any]) -> Dict[str, Any]:
        """Update auto-accept settings.

        Args:
            settings: Auto-accept settings

        Returns:
            Updated settings
        """
        return await self._make_request("PUT", "/api/config/auto-accept", json_data=settings)

    async def get_player_watch_config(self) -> Dict[str, Any]:
        """Get player watch configuration.

        Returns:
            Player watch config
        """
        return await self._make_request("GET", "/api/config/player-watch")

    async def set_player_watch_config(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set player watch configuration.

        Args:
            config: Configuration

        Returns:
            Updated config
        """
        return await self._make_request("PUT", "/api/config/player-watch", json_data=config)

    # ─── Timestamp Config ───────────────────────────────────────────────

    async def list_timestamp_configs(self) -> List[Dict[str, Any]]:
        """List all timestamp configurations.

        Returns:
            List of timestamp configs
        """
        return await self._make_request("GET", "/api/timestamp-config")

    async def get_timestamp_config(self, character: str) -> Dict[str, Any]:
        """Get timestamp configuration for character.

        Args:
            character: Character name

        Returns:
            Timestamp config
        """
        return await self._make_request("GET", f"/api/timestamp-config/{character}")

    async def set_timestamp_config(self, character: str, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set timestamp configuration for character.

        Args:
            character: Character name
            config: Configuration

        Returns:
            Updated config
        """
        return await self._make_request("PUT", f"/api/timestamp-config/{character}", json_data=config)

    # ─── Kill Tracker ───────────────────────────────────────────────────

    async def list_kill_tracker_records(self) -> List[Dict[str, Any]]:
        """List all kill tracker records.

        Returns:
            List of kill records
        """
        return await self._make_request("GET", "/api/kill-tracker/sessions")

    async def get_kill_tracker_stats(self) -> Dict[str, Any]:
        """Get kill tracker statistics.

        Returns:
            Kill tracker stats
        """
        history = await self._make_request("GET", "/api/kill-tracker/history")
        return self._summarize_kill_tracker_history(history)

    async def get_kill_tracker_settings(self) -> Dict[str, Any]:
        """Get kill tracker settings."""
        return await self._make_request("GET", "/api/kill-tracker/settings")

    async def set_kill_tracker_settings(self, settings: Dict[str, Any]) -> Dict[str, Any]:
        """Update kill tracker settings."""
        return await self._make_request("PUT", "/api/kill-tracker/settings", json_data=settings)

    async def list_kill_tracker_sessions(self) -> List[Dict[str, Any]]:
        """List the latest kill tracker session for each character."""
        return await self._make_request("GET", "/api/kill-tracker/sessions")

    async def get_kill_tracker_character_sessions(self, character: str) -> List[Dict[str, Any]]:
        """Get all kill tracker sessions for a single character."""
        return await self._make_request("GET", f"/api/kill-tracker/sessions/{character}")

    async def get_kill_tracker_history(self) -> List[Dict[str, Any]]:
        """Get the kill tracker history grouped by character."""
        return await self._make_request("GET", "/api/kill-tracker/history")

    async def clear_kill_tracker(self) -> None:
        """Clear all kill tracker records."""
        client = await self._ensure_client()
        response = await client.delete("/api/kill-tracker/records")
        response.raise_for_status()

    # ─── XAssist ────────────────────────────────────────────────────────

    async def list_xassist_configs(self) -> List[Dict[str, Any]]:
        """List all XAssist configurations.

        Returns:
            List of XAssist configs
        """
        return await self._make_request("GET", "/api/xassist/configs")

    async def get_xassist_config(self, character: str) -> Dict[str, Any]:
        """Get XAssist configuration for character.

        Args:
            character: Character name

        Returns:
            XAssist config
        """
        return await self._make_request("GET", f"/api/xassist/config/{character}")

    async def set_xassist_config(self, character: str, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set XAssist configuration for character.

        Args:
            character: Character name
            config: Configuration

        Returns:
            Updated config
        """
        return await self._make_request("PUT", f"/api/xassist/config/{character}", json_data=config)

    async def delete_xassist_config(self, character: str) -> None:
        """Delete XAssist configuration for character.

        Args:
            character: Character name
        """
        client = await self._ensure_client()
        response = await client.delete(f"/api/xassist/config/{character}")
        response.raise_for_status()

    # ─── Chat Pattern Rules ─────────────────────────────────────────────

    async def list_chat_pattern_rules(self) -> List[Dict[str, Any]]:
        """List all chat pattern rules.

        Returns:
            List of rules
        """
        return await self._make_request("GET", "/api/chat-pattern-rules")

    async def get_chat_pattern_rule(self, rule_id: str) -> Dict[str, Any]:
        """Get specific chat pattern rule.

        Args:
            rule_id: Rule ID

        Returns:
            Rule details
        """
        return await self._make_request("GET", f"/api/chat-pattern-rules/{rule_id}")

    async def create_chat_pattern_rule(self, rule_data: Dict[str, Any]) -> Dict[str, Any]:
        """Create a new chat pattern rule.

        Args:
            rule_data: Rule configuration

        Returns:
            Created rule
        """
        return await self._make_request("POST", "/api/chat-pattern-rules", json_data=rule_data)

    async def update_chat_pattern_rule(self, rule_id: str, rule_data: Dict[str, Any]) -> Dict[str, Any]:
        """Update a chat pattern rule.

        Args:
            rule_id: Rule ID
            rule_data: Updated rule data

        Returns:
            Updated rule
        """
        return await self._make_request("PUT", f"/api/chat-pattern-rules/{rule_id}", json_data=rule_data)

    async def delete_chat_pattern_rule(self, rule_id: str) -> None:
        """Delete a chat pattern rule.

        Args:
            rule_id: Rule ID
        """
        client = await self._ensure_client()
        response = await client.delete(f"/api/chat-pattern-rules/{rule_id}")
        response.raise_for_status()

    async def toggle_chat_pattern_rule(self, rule_id: str, enabled: bool) -> Dict[str, Any]:
        """Toggle a chat pattern rule on/off.

        Args:
            rule_id: Rule ID
            enabled: Whether to enable

        Returns:
            Updated rule
        """
        return await self._make_request("PUT", f"/api/chat-pattern-rules/{rule_id}/toggle", json_data={"enabled": enabled})

    async def get_chat_pattern_rules_stats(self) -> Dict[str, Any]:
        """Get chat pattern rules statistics.

        Returns:
            Rules stats
        """
        return await self._make_request("GET", "/api/chat-pattern-rules/stats")

    async def import_chat_pattern_rules(self, rules: List[Dict[str, Any]]) -> Dict[str, Any]:
        """Import chat pattern rules.

        Args:
            rules: List of rules to import

        Returns:
            Import result
        """
        return await self._make_request("POST", "/api/chat-pattern-rules/import", json_data={"rules": rules})

    async def reset_chat_pattern_rule_cooldown(self, rule_id: str) -> Dict[str, Any]:
        """Reset cooldown for a chat pattern rule.

        Args:
            rule_id: Rule ID

        Returns:
            Updated rule
        """
        return await self._make_request("PUT", f"/api/chat-pattern-rules/{rule_id}/reset-cooldown")

    async def reset_all_chat_pattern_cooldowns(self) -> Dict[str, Any]:
        """Reset all chat pattern rule cooldowns.

        Returns:
            Status message
        """
        return await self._make_request("PUT", "/api/chat-pattern-rules/cooldowns/reset")

    # ─── Say Detection ───────────────────────────────────────────────────

    async def get_say_detection_config(self) -> Dict[str, Any]:
        """Get say detection configuration.

        Returns:
            Say detection config
        """
        return await self._make_request("GET", "/api/say-detection/config")

    async def set_say_detection_config(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set say detection configuration.

        Args:
            config: Configuration

        Returns:
            Updated config
        """
        return await self._make_request("PUT", "/api/say-detection/config", json_data=config)

    async def close(self) -> None:
        """Close the client session."""
        if self.client:
            await self.client.aclose()

    async def __aenter__(self):
        """Async context manager entry."""
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.close()
