"""Synchronous TextQuest API client.

This module provides a sync wrapper around the TextQuest web API using requests.
"""

from typing import Any, Dict, List, Optional
import json
from urllib.parse import quote
import requests
from requests.exceptions import RequestException

from textquest_sdk.utils import summarize_kill_tracker_history

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


class TextQuestClient:
    """Synchronous client for the TextQuest Web API.

    Example:
        >>> client = TextQuestClient("http://localhost:3001")
        >>> health = client.health()
        >>> sessions = client.list_sessions()
    """

    def __init__(self, base_url: str = "http://localhost:3001", api_token: Optional[str] = None):
        """Initialize the TextQuest client.

        Args:
            base_url: The base URL of the TextQuest web API (default: http://localhost:3001)
            api_token: Optional API token for authentication (X-API-Token header)
        """
        self.base_url = base_url.rstrip("/")
        self.session = requests.Session()
        if api_token:
            self.session.headers.update({"X-API-Token": api_token})

    def _make_request(
        self,
        method: str,
        endpoint: str,
        json_data: Optional[Dict[str, Any]] = None,
        params: Optional[Dict[str, Any]] = None,
    ) -> Any:
        """Make an HTTP request to the API.

        Args:
            method: HTTP method (GET, POST, PUT, DELETE)
            endpoint: API endpoint path
            json_data: JSON body data
            params: Query parameters

        Returns:
            Parsed JSON response

        Raises:
            RequestException: On network or HTTP errors
        """
        url = f"{self.base_url}{endpoint}"
        response = self.session.request(
            method, url, json=json_data, params=params, timeout=10
        )
        response.raise_for_status()

        if response.text:
            return response.json()
        return None


    # ─── Health & Status ─────────────────────────────────────────────────

    def health(self) -> Dict[str, str]:
        """Get API health status.

        Returns:
            Health status with version information
        """
        return self._make_request("GET", "/api/health")

    def list_sessions(self) -> List[Dict[str, Any]]:
        """List all active sessions.

        Returns:
            List of session information
        """
        return self._make_request("GET", "/api/sessions")

    # ─── Accounts ───────────────────────────────────────────────────────

    def create_account(self, account_data: Dict[str, Any]) -> Dict[str, Any]:
        """Create a new account.

        Args:
            account_data: Account information (name, server, character, class, group, status, password)

        Returns:
            Created account details
        """
        return self._make_request("POST", "/api/accounts", json_data=account_data)

    def list_accounts(self) -> List[Dict[str, Any]]:
        """List all accounts.

        Returns:
            List of account information
        """
        return self._make_request("GET", "/api/accounts")

    def get_account(self, name: str) -> Dict[str, Any]:
        """Get account details by name.

        Args:
            name: Account name

        Returns:
            Account information
        """
        return self._make_request("GET", f"/api/accounts/{name}")

    def update_account(self, name: str, account_data: Dict[str, Any]) -> Dict[str, Any]:
        """Update account information.

        Args:
            name: Account name
            account_data: Updated account fields

        Returns:
            Updated account details
        """
        return self._make_request("PUT", f"/api/accounts/{name}", json_data=account_data)

    def set_account_password(self, name: str, password: str) -> None:
        """Set or update account password.

        Args:
            name: Account name
            password: New password

        Raises:
            RequestException: If credential store is not enabled
        """
        self._make_request("PUT", f"/api/accounts/{name}/password", json_data={"password": password})

    def delete_account(self, name: str) -> None:
        """Delete an account.

        Args:
            name: Account name
        """
        self.session.delete(f"{self.base_url}/api/accounts/{name}", timeout=10).raise_for_status()

    def export_accounts(self) -> Dict[str, Any]:
        """Export all accounts.

        Returns:
            Exported accounts data
        """
        return self._make_request("GET", "/api/accounts/export")

    def import_accounts(self, accounts_data: List[Dict[str, Any]]) -> Dict[str, Any]:
        """Import accounts.

        Args:
            accounts_data: List of account data to import

        Returns:
            Import result with count
        """
        return self._make_request("POST", "/api/accounts/import", json_data={"accounts": accounts_data})

    # ─── Economy ────────────────────────────────────────────────────────

    def get_economy_settings(self) -> Dict[str, Any]:
        """Get economy configuration.

        Returns:
            Economy settings
        """
        return self._make_request("GET", "/api/economy/settings")

    def set_economy_settings(self, settings: Dict[str, Any]) -> Dict[str, Any]:
        """Update economy configuration.

        Args:
            settings: Economy settings to update

        Returns:
            Updated economy settings
        """
        return self._make_request("PUT", "/api/economy/settings", json_data=settings)

    def list_vendor_routes(self) -> List[Dict[str, Any]]:
        """List all vendor routes.

        Returns:
            List of vendor routes
        """
        return self._make_request("GET", "/api/economy/vendor-routes")

    def create_vendor_route(self, route_data: Dict[str, Any]) -> Dict[str, Any]:
        """Create a new vendor route.

        Args:
            route_data: Route information (name, vendors, priority)

        Returns:
            Created route details
        """
        return self._make_request("POST", "/api/economy/vendor-routes", json_data=route_data)

    def update_vendor_route(self, route_id: str, route_data: Dict[str, Any]) -> Dict[str, Any]:
        """Update a vendor route.

        Args:
            route_id: Route ID
            route_data: Updated route fields

        Returns:
            Updated route details
        """
        return self._make_request("PUT", f"/api/economy/vendor-routes/{route_id}", json_data=route_data)

    def delete_vendor_route(self, route_id: str) -> None:
        """Delete a vendor route.

        Args:
            route_id: Route ID
        """
        self.session.delete(f"{self.base_url}/api/economy/vendor-routes/{route_id}", timeout=10).raise_for_status()

    def get_wealth(self) -> Dict[str, Any]:
        """Get current wealth snapshot.

        Returns:
            Wealth information for all characters
        """
        return self._make_request("GET", "/api/economy/wealth")

    # ─── Soul States ────────────────────────────────────────────────────

    def list_soul_states(self) -> List[Dict[str, Any]]:
        """List all character soul states.

        Returns:
            List of soul state information
        """
        return self._make_request("GET", "/api/soul")

    def get_soul_state(self, character_id: str) -> Dict[str, Any]:
        """Get soul state for a character.

        Args:
            character_id: Character ID

        Returns:
            Soul state information
        """
        return self._make_request("GET", f"/api/soul/{character_id}")

    def get_soul_audit(self) -> List[Dict[str, Any]]:
        """Get all soul audit entries.

        Returns:
            List of audit entries
        """
        return self._make_request("GET", "/api/soul/audit")

    def get_character_audit(self, character_id: str) -> List[Dict[str, Any]]:
        """Get soul audit entries for a character.

        Args:
            character_id: Character ID

        Returns:
            List of audit entries for the character
        """
        return self._make_request("GET", f"/api/soul/audit/{character_id}")

    def export_audit_csv(self) -> str:
        """Export all audit data as CSV.

        Returns:
            CSV content
        """
        response = self.session.get(f"{self.base_url}/api/soul/audit/export.csv", timeout=10)
        response.raise_for_status()
        return response.text

    def export_character_audit_csv(self, character_id: str) -> str:
        """Export character audit data as CSV.

        Args:
            character_id: Character ID

        Returns:
            CSV content
        """
        response = self.session.get(
            f"{self.base_url}/api/soul/audit/{character_id}/export.csv", timeout=10
        )
        response.raise_for_status()
        return response.text

    # ─── Loot Management ────────────────────────────────────────────────

    def get_loot_rules(self) -> Dict[str, Any]:
        """Get loot distribution rules.

        Returns:
            Loot rules configuration
        """
        return self._make_request("GET", "/api/loot/rules")

    def set_loot_rules(self, rules: Dict[str, Any]) -> Dict[str, Any]:
        """Update loot distribution rules.

        Args:
            rules: Loot rules configuration

        Returns:
            Updated loot rules
        """
        return self._make_request("PUT", "/api/loot/rules", json_data=rules)

    def get_loot_filters(self) -> List[Dict[str, Any]]:
        """Get all loot filters.

        Returns:
            List of loot filters
        """
        return self._make_request("GET", "/api/loot/filters")

    def set_loot_filter(self, character: str, filter_data: Dict[str, Any]) -> Dict[str, Any]:
        """Set loot filter for a character.

        Args:
            character: Character name
            filter_data: Filter configuration

        Returns:
            Updated filter
        """
        return self._make_request("PUT", f"/api/loot/filters/{character}", json_data=filter_data)

    def get_master_looter(self) -> Dict[str, Any]:
        """Get master looter configuration.

        Returns:
            Master looter information
        """
        return self._make_request("GET", "/api/loot/master-looter")

    def set_master_looter(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set master looter configuration.

        Args:
            config: Master looter configuration

        Returns:
            Updated configuration
        """
        return self._make_request("PUT", "/api/loot/master-looter", json_data=config)

    def get_loot_distribution(self) -> Dict[str, Any]:
        """Get loot distribution configuration.

        Returns:
            Distribution configuration
        """
        return self._make_request("GET", "/api/loot/distribution")

    def set_loot_distribution(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set loot distribution configuration.

        Args:
            config: Distribution configuration

        Returns:
            Updated configuration
        """
        return self._make_request("PUT", "/api/loot/distribution", json_data=config)

    def get_loot_history(self) -> List[Dict[str, Any]]:
        """Get loot distribution history.

        Returns:
            List of historical loot entries
        """
        return self._make_request("GET", "/api/loot/history")

    # ─── Alerts ─────────────────────────────────────────────────────────

    def get_alert_config(self) -> Dict[str, Any]:
        """Get alert configuration.

        Returns:
            Alert configuration
        """
        response = self._make_request("GET", "/api/alerts/config")
        return response["config"]

    def set_alert_config(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Update alert configuration.

        Args:
            config: Alert configuration

        Returns:
            Updated configuration
        """
        response = self._make_request("PUT", "/api/alerts/config", json_data=config)
        return response["config"]

    def list_alerts(self) -> Dict[str, Any]:
        """List all alerts.

        Returns:
            Alert envelope with alerts and unread count
        """
        return self._make_request("GET", "/api/alerts")

    def get_alert(self, alert_id: str) -> Dict[str, Any]:
        """Get specific alert.

        Args:
            alert_id: Alert ID

        Returns:
            Alert details
        """
        return self._make_request("GET", f"/api/alerts/{alert_id}")

    # ─── Spawn Alerts ───────────────────────────────────────────────────

    def list_spawn_alerts(self) -> List[Dict[str, Any]]:
        """List spawn alerts.

        Returns:
            List of spawn alerts
        """
        return self._make_request("GET", "/api/spawn-alerts")

    def clear_spawn_alerts(self) -> None:
        """Clear all spawn alerts."""
        self.session.delete(f"{self.base_url}/api/spawn-alerts", timeout=10).raise_for_status()

    def get_spawn_alert_stats(self) -> Dict[str, Any]:
        """Get spawn alert statistics.

        Returns:
            Spawn alert stats
        """
        return self._make_request("GET", "/api/spawn-alerts/stats")

    def get_spawn_alert_config(self) -> Dict[str, Any]:
        """Get spawn alert configuration.

        Returns:
            Spawn alert config
        """
        return self._make_request("GET", "/api/spawn-alerts/config")

    def set_spawn_alert_config(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set spawn alert configuration.

        Args:
            config: Configuration

        Returns:
            Updated config
        """
        return self._make_request("PUT", "/api/spawn-alerts/config", json_data=config)

    def get_spawn_watch_list(self) -> List[str]:
        """Get spawn watch list.

        Returns:
            List of watched spawn patterns
        """
        return self._make_request("GET", "/api/spawn-alerts/watch-list")

    def add_spawn_watch_pattern(self, pattern: str) -> Dict[str, Any]:
        """Add a spawn watch pattern.

        Args:
            pattern: Pattern to watch

        Returns:
            Updated watch list
        """
        return self._make_request("PUT", "/api/spawn-alerts/watch-list", json_data={"pattern": pattern})

    def remove_spawn_watch_pattern(self, pattern: str) -> None:
        """Remove a spawn watch pattern.

        Args:
            pattern: Pattern to remove
        """
        encoded_pattern = quote(pattern, safe="")
        self.session.delete(
            f"{self.base_url}/api/spawn-alerts/watch-list/{encoded_pattern}", timeout=10
        ).raise_for_status()

    # ─── GM Alerts ───────────────────────────────────────────────────────

    def list_gm_alerts(self) -> Dict[str, Any]:
        """Get the live GM alert status.

        Returns:
            GM alert status envelope
        """
        return self.get_gm_alert_status()

    def get_gm_alert_state(self) -> Dict[str, Any]:
        """Get current GM alert state.

        Returns:
            GM alert state
        """
        return self.get_gm_alert_status()

    def get_gm_alert_config(self) -> Dict[str, Any]:
        """Get the persisted GM alert configuration."""
        return self._make_request("GET", "/api/gm-alerts/config")

    def set_gm_alert_config(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Update the persisted GM alert configuration."""
        return self._make_request("PUT", "/api/gm-alerts/config", json_data=config)

    def get_gm_alert_status(self) -> Dict[str, Any]:
        """Get the live GM alert status."""
        return self._make_request("GET", "/api/gm-alerts/status")

    def sync_gm_alert_status(self, payload: Dict[str, Any]) -> Dict[str, Any]:
        """Push live GM alert status into the server-side cache."""
        return self._make_request("POST", "/api/gm-alerts/sync", json_data=payload)

    def acknowledge_gm_alert(self, alert_id: str) -> None:
        """Acknowledge a GM alert.

        Args:
            alert_id: Alert ID
        """
        self.session.put(f"{self.base_url}/api/gm-alerts/{alert_id}/acknowledge", timeout=10).raise_for_status()

    # ─── Character Config ───────────────────────────────────────────────

    def list_character_configs(self) -> List[Dict[str, Any]]:
        """List all character configurations.

        Returns:
            List of character configs
        """
        return self._make_request("GET", "/api/config/characters")

    def set_character_config(self, character: str, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set character configuration.

        Args:
            character: Character name
            config: Configuration data

        Returns:
            Updated config
        """
        return self._make_request("PUT", f"/api/config/characters/{character}", json_data=config)

    def get_auto_accept_settings(self) -> Dict[str, Any]:
        """Get auto-accept settings.

        Returns:
            Auto-accept configuration
        """
        return self._make_request("GET", "/api/config/auto-accept")

    def set_auto_accept_settings(self, settings: Dict[str, Any]) -> Dict[str, Any]:
        """Update auto-accept settings.

        Args:
            settings: Auto-accept settings

        Returns:
            Updated settings
        """
        return self._make_request("PUT", "/api/config/auto-accept", json_data=settings)

    def get_player_watch_config(self) -> Dict[str, Any]:
        """Get player watch configuration.

        Returns:
            Player watch config
        """
        return self._make_request("GET", "/api/config/player-watch")

    def set_player_watch_config(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set player watch configuration.

        Args:
            config: Configuration

        Returns:
            Updated config
        """
        return self._make_request("PUT", "/api/config/player-watch", json_data=config)

    # ─── Timestamp Config ───────────────────────────────────────────────

    def list_timestamp_configs(self) -> List[Dict[str, Any]]:
        """List all timestamp configurations.

        Returns:
            List of timestamp configs
        """
        return self._make_request("GET", "/api/timestamp-config")

    def get_timestamp_config(self, character: str) -> Dict[str, Any]:
        """Get timestamp configuration for character.

        Args:
            character: Character name

        Returns:
            Timestamp config
        """
        return self._make_request("GET", f"/api/timestamp-config/{character}")

    def set_timestamp_config(self, character: str, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set timestamp configuration for character.

        Args:
            character: Character name
            config: Configuration

        Returns:
            Updated config
        """
        return self._make_request("PUT", f"/api/timestamp-config/{character}", json_data=config)

    # ─── Kill Tracker ───────────────────────────────────────────────────

    def list_kill_tracker_records(self) -> List[Dict[str, Any]]:
        """List all kill tracker records.

        Returns:
            List of kill records
        """
        return self._make_request("GET", "/api/kill-tracker/sessions")

    def get_kill_tracker_stats(self) -> Dict[str, Any]:
        """Get kill tracker statistics.

        Returns:
            Kill tracker stats
        """
        history = self._make_request("GET", "/api/kill-tracker/history")
        return summarize_kill_tracker_history(history)

    def get_kill_tracker_settings(self) -> Dict[str, Any]:
        """Get kill tracker settings."""
        return self._make_request("GET", "/api/kill-tracker/settings")

    def set_kill_tracker_settings(self, settings: Dict[str, Any]) -> Dict[str, Any]:
        """Update kill tracker settings."""
        return self._make_request("PUT", "/api/kill-tracker/settings", json_data=settings)

    def list_kill_tracker_sessions(self) -> List[Dict[str, Any]]:
        """List the latest kill tracker session for each character."""
        return self._make_request("GET", "/api/kill-tracker/sessions")

    def get_kill_tracker_character_sessions(self, character: str) -> List[Dict[str, Any]]:
        """Get all kill tracker sessions for a single character."""
        return self._make_request("GET", f"/api/kill-tracker/sessions/{character}")

    def get_kill_tracker_history(self) -> List[Dict[str, Any]]:
        """Get the kill tracker history grouped by character."""
        return self._make_request("GET", "/api/kill-tracker/history")

    def clear_kill_tracker(self) -> None:
        """Clear all kill tracker records."""
        self.session.delete(f"{self.base_url}/api/kill-tracker/records", timeout=10).raise_for_status()

    # ─── XAssist ────────────────────────────────────────────────────────

    def list_xassist_configs(self) -> List[Dict[str, Any]]:
        """List all XAssist configurations.

        Returns:
            List of XAssist configs
        """
        return self._make_request("GET", "/api/xassist/configs")

    def get_xassist_config(self, character: str) -> Dict[str, Any]:
        """Get XAssist configuration for character.

        Args:
            character: Character name

        Returns:
            XAssist config
        """
        return self._make_request("GET", f"/api/xassist/config/{character}")

    def set_xassist_config(self, character: str, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set XAssist configuration for character.

        Args:
            character: Character name
            config: Configuration

        Returns:
            Updated config
        """
        return self._make_request("PUT", f"/api/xassist/config/{character}", json_data=config)

    def delete_xassist_config(self, character: str) -> None:
        """Delete XAssist configuration for character.

        Args:
            character: Character name
        """
        self.session.delete(
            f"{self.base_url}/api/xassist/config/{character}", timeout=10
        ).raise_for_status()

    # ─── Chat Pattern Rules ─────────────────────────────────────────────

    def list_chat_pattern_rules(self) -> List[Dict[str, Any]]:
        """List all chat pattern rules.

        Returns:
            List of rules
        """
        return self._make_request("GET", "/api/chat-pattern-rules")

    def get_chat_pattern_rule(self, rule_id: str) -> Dict[str, Any]:
        """Get specific chat pattern rule.

        Args:
            rule_id: Rule ID

        Returns:
            Rule details
        """
        return self._make_request("GET", f"/api/chat-pattern-rules/{rule_id}")

    def create_chat_pattern_rule(self, rule_data: Dict[str, Any]) -> Dict[str, Any]:
        """Create a new chat pattern rule.

        Args:
            rule_data: Rule configuration

        Returns:
            Created rule
        """
        return self._make_request("POST", "/api/chat-pattern-rules", json_data=rule_data)

    def update_chat_pattern_rule(self, rule_id: str, rule_data: Dict[str, Any]) -> Dict[str, Any]:
        """Update a chat pattern rule.

        Args:
            rule_id: Rule ID
            rule_data: Updated rule data

        Returns:
            Updated rule
        """
        return self._make_request("PUT", f"/api/chat-pattern-rules/{rule_id}", json_data=rule_data)

    def delete_chat_pattern_rule(self, rule_id: str) -> None:
        """Delete a chat pattern rule.

        Args:
            rule_id: Rule ID
        """
        self.session.delete(
            f"{self.base_url}/api/chat-pattern-rules/{rule_id}", timeout=10
        ).raise_for_status()

    def toggle_chat_pattern_rule(self, rule_id: str, enabled: bool) -> Dict[str, Any]:
        """Toggle a chat pattern rule on/off.

        Args:
            rule_id: Rule ID
            enabled: Whether to enable

        Returns:
            Updated rule
        """
        return self._make_request("PUT", f"/api/chat-pattern-rules/{rule_id}/toggle", json_data={"enabled": enabled})

    def get_chat_pattern_rules_stats(self) -> Dict[str, Any]:
        """Get chat pattern rules statistics.

        Returns:
            Rules stats
        """
        return self._make_request("GET", "/api/chat-pattern-rules/stats")

    def import_chat_pattern_rules(self, rules: List[Dict[str, Any]]) -> Dict[str, Any]:
        """Import chat pattern rules.

        Args:
            rules: List of rules to import

        Returns:
            Import result
        """
        return self._make_request("POST", "/api/chat-pattern-rules/import", json_data={"rules": rules})

    def reset_chat_pattern_rule_cooldown(self, rule_id: str) -> Dict[str, Any]:
        """Reset cooldown for a chat pattern rule.

        Args:
            rule_id: Rule ID

        Returns:
            Updated rule
        """
        return self._make_request("PUT", f"/api/chat-pattern-rules/{rule_id}/reset-cooldown")

    def reset_all_chat_pattern_cooldowns(self) -> Dict[str, Any]:
        """Reset all chat pattern rule cooldowns.

        Returns:
            Status message
        """
        return self._make_request("PUT", "/api/chat-pattern-rules/cooldowns/reset")

    # ─── Say Detection ───────────────────────────────────────────────────

    def get_say_detection_config(self) -> Dict[str, Any]:
        """Get say detection configuration.

        Returns:
            Say detection config
        """
        return self._make_request("GET", "/api/say-detection/config")

    def set_say_detection_config(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Set say detection configuration.

        Args:
            config: Configuration

        Returns:
            Updated config
        """
        return self._make_request("PUT", "/api/say-detection/config", json_data=config)

    def close(self) -> None:
        """Close the client session."""
        self.session.close()

    def __enter__(self):
        """Context manager entry."""
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit."""
        self.close()
