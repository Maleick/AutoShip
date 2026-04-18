"""Tests for the synchronous TextQuest client."""

import pytest
from unittest.mock import Mock, patch, MagicMock
from textquest_sdk import TextQuestClient


class TestTextQuestClient:
    """Test cases for the TextQuestClient."""

    @pytest.fixture
    def mock_session(self):
        """Create a mock requests session."""
        with patch('textquest_sdk.client.requests.Session') as mock:
            yield mock.return_value

    @pytest.fixture
    def client(self, mock_session):
        """Create a client instance with mocked session."""
        return TextQuestClient("http://localhost:3001")

    def test_client_initialization(self):
        """Test client initialization."""
        client = TextQuestClient("http://example.com", api_token="test-token")
        assert client.base_url == "http://example.com"
        assert "X-API-Token" in client.session.headers

    def test_health_endpoint(self, client, mock_session):
        """Test health check endpoint."""
        mock_response = Mock()
        mock_response.text = '{"status": "ok", "version": "0.1.0"}'
        mock_response.json.return_value = {"status": "ok", "version": "0.1.0"}
        mock_session.request.return_value = mock_response

        result = client.health()

        assert result["status"] == "ok"
        assert result["version"] == "0.1.0"
        mock_session.request.assert_called_once()

    def test_list_sessions(self, client, mock_session):
        """Test listing sessions."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []
        mock_session.request.return_value = mock_response

        result = client.list_sessions()

        assert result == []
        mock_session.request.assert_called_once()

    def test_create_account(self, client, mock_session):
        """Test account creation."""
        account_data = {
            "name": "test_account",
            "server": "Teek",
            "character": "TestChar",
            "class": "WIZ",
            "group": 1,
            "status": "active",
            "password": "secret"
        }
        mock_response = Mock()
        mock_response.text = '{"name": "test_account"}'
        mock_response.json.return_value = {"name": "test_account"}
        mock_session.request.return_value = mock_response

        result = client.create_account(account_data)

        assert result["name"] == "test_account"
        mock_session.request.assert_called_once()

    def test_list_accounts(self, client, mock_session):
        """Test listing accounts."""
        mock_response = Mock()
        mock_response.text = '[{"name": "account1"}]'
        mock_response.json.return_value = [{"name": "account1"}]
        mock_session.request.return_value = mock_response

        result = client.list_accounts()

        assert len(result) == 1
        assert result[0]["name"] == "account1"

    def test_get_account(self, client, mock_session):
        """Test getting a specific account."""
        mock_response = Mock()
        mock_response.text = '{"name": "test_account"}'
        mock_response.json.return_value = {"name": "test_account"}
        mock_session.request.return_value = mock_response

        result = client.get_account("test_account")

        assert result["name"] == "test_account"

    def test_update_account(self, client, mock_session):
        """Test updating an account."""
        update_data = {"group": 2}
        mock_response = Mock()
        mock_response.text = '{"name": "test_account", "group": 2}'
        mock_response.json.return_value = {"name": "test_account", "group": 2}
        mock_session.request.return_value = mock_response

        result = client.update_account("test_account", update_data)

        assert result["group"] == 2

    def test_delete_account(self, client, mock_session):
        """Test deleting an account."""
        mock_response = Mock()
        mock_response.raise_for_status = Mock()
        mock_session.delete.return_value = mock_response

        client.delete_account("test_account")

        mock_session.delete.assert_called_once()

    def test_economy_settings(self, client, mock_session):
        """Test economy settings endpoints."""
        mock_response = Mock()
        mock_response.text = '{"krono_farming_enabled": true}'
        mock_response.json.return_value = {"krono_farming_enabled": True}
        mock_session.request.return_value = mock_response

        result = client.get_economy_settings()
        assert result["krono_farming_enabled"] is True

        client.set_economy_settings({"krono_farming_enabled": False})
        assert mock_session.request.call_count == 2

    def test_vendor_routes(self, client, mock_session):
        """Test vendor route operations."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []
        mock_session.request.return_value = mock_response

        result = client.list_vendor_routes()
        assert result == []

    def test_wealth_endpoint(self, client, mock_session):
        """Test wealth endpoint."""
        mock_response = Mock()
        mock_response.text = '{"total_platinum": 5000}'
        mock_response.json.return_value = {"total_platinum": 5000}
        mock_session.request.return_value = mock_response

        result = client.get_wealth()
        assert result["total_platinum"] == 5000

    def test_soul_states(self, client, mock_session):
        """Test soul state endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []
        mock_session.request.return_value = mock_response

        result = client.list_soul_states()
        assert result == []

    def test_loot_rules(self, client, mock_session):
        """Test loot rules endpoints."""
        mock_response = Mock()
        mock_response.text = '{"auto_split_stackables": true}'
        mock_response.json.return_value = {"auto_split_stackables": True}
        mock_session.request.return_value = mock_response

        result = client.get_loot_rules()
        assert result["auto_split_stackables"] is True

    def test_spawn_alerts(self, client, mock_session):
        """Test spawn alert endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []
        mock_session.request.return_value = mock_response

        result = client.list_spawn_alerts()
        assert result == []

    def test_character_configs(self, client, mock_session):
        """Test character configuration endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []
        mock_session.request.return_value = mock_response

        result = client.list_character_configs()
        assert result == []

    def test_alert_config(self, client, mock_session):
        """Test alert configuration endpoints."""
        mock_response = Mock()
        mock_response.text = '{"config": {"enabled": true}}'
        mock_response.json.return_value = {"config": {"enabled": True}}
        mock_session.request.return_value = mock_response

        result = client.get_alert_config()

        mock_session.request.assert_called_once_with(
            "GET",
            "http://localhost:3001/api/alerts/config",
            json=None,
            params=None,
            timeout=10,
        )
        assert result["enabled"] is True

    def test_spawn_watch_pattern_request_shape(self, client, mock_session):
        """Test that spawn watch updates send the expected JSON body."""
        mock_response = Mock()
        mock_response.text = ""
        mock_response.json.return_value = None
        mock_session.request.return_value = mock_response

        result = client.add_spawn_watch_pattern("Ancient Dragon")

        mock_session.request.assert_called_once_with(
            "PUT",
            "http://localhost:3001/api/spawn-alerts/watch-list",
            json={"pattern": "Ancient Dragon"},
            params=None,
            timeout=10,
        )
        assert result is None

    def test_gm_alert_status_routes(self, client, mock_session):
        """Test that GM alert helpers hit the live status endpoint."""
        mock_response = Mock()
        mock_response.text = '{"config": {"enabled": true}, "presence": {"is_gm_in_zone": false, "gm_count": 0, "gm_names": []}, "automation_paused": false}'
        mock_response.json.return_value = {
            "config": {"enabled": True},
            "presence": {"is_gm_in_zone": False, "gm_count": 0, "gm_names": []},
            "automation_paused": False,
        }
        mock_session.request.return_value = mock_response

        result = client.get_gm_alert_state()

        mock_session.request.assert_called_once_with(
            "GET",
            "http://localhost:3001/api/gm-alerts/status",
            json=None,
            params=None,
            timeout=10,
        )
        assert result["presence"]["is_gm_in_zone"] is False

    def test_kill_tracker_stats_are_derived_from_history(self, client, mock_session):
        """Test that kill tracker stats are derived from history data."""
        history_response = Mock()
        history_response.text = '[{"sessions": [{"total_kills": 4, "zone": "Dreadlands"}, {"total_kills": 2, "zone": "Dreadlands"}]}]'
        history_response.json.return_value = [
            {
                "sessions": [
                    {"total_kills": 4, "zone": "Dreadlands"},
                    {"total_kills": 2, "zone": "Dreadlands"},
                ]
            }
        ]

        def request_side_effect(method, url, json=None, params=None, timeout=None):
            if url.endswith("/api/kill-tracker/history"):
                return history_response
            records_response = Mock()
            records_response.text = "[]"
            records_response.json.return_value = []
            return records_response

        mock_session.request.side_effect = request_side_effect

        records = client.list_kill_tracker_records()
        stats = client.get_kill_tracker_stats()

        assert records == []
        assert stats["total_kills"] == 6
        assert stats["kills_by_zone"]["Dreadlands"] == 6

    def test_chat_pattern_rules(self, client, mock_session):
        """Test chat pattern rules endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []
        mock_session.request.return_value = mock_response

        result = client.list_chat_pattern_rules()
        assert result == []

    def test_context_manager(self):
        """Test context manager usage."""
        with patch('textquest_sdk.client.requests.Session'):
            with TextQuestClient("http://localhost:3001") as client:
                assert client is not None
                assert hasattr(client, 'session')

    def test_api_token_in_headers(self):
        """Test that API token is properly set in headers."""
        with patch('textquest_sdk.client.requests.Session') as mock_session_class:
            mock_session_instance = Mock()
            mock_session_class.return_value = mock_session_instance
            client = TextQuestClient("http://localhost:3001", api_token="secret-token")
            mock_session_instance.headers.update.assert_called_once_with({"X-API-Token": "secret-token"})

    def test_url_normalization(self):
        """Test that trailing slashes are removed from base URL."""
        with patch('textquest_sdk.client.requests.Session'):
            client = TextQuestClient("http://localhost:3001/")
            assert client.base_url == "http://localhost:3001"

    def test_empty_response_handling(self, client, mock_session):
        """Test handling of empty responses (e.g., DELETE operations)."""
        mock_response = Mock()
        mock_response.text = ""
        mock_session.request.return_value = mock_response

        result = client._make_request("DELETE", "/api/test")
        assert result is None

    def test_kill_tracker_operations(self, client, mock_session):
        """Test kill tracker endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []
        mock_session.request.return_value = mock_response

        result = client.list_kill_tracker_records()
        assert result == []

    def test_xassist_config(self, client, mock_session):
        """Test XAssist configuration endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []
        mock_session.request.return_value = mock_response

        result = client.list_xassist_configs()
        assert result == []

    def test_say_detection(self, client, mock_session):
        """Test say detection endpoints."""
        mock_response = Mock()
        mock_response.text = '{"enabled": false}'
        mock_response.json.return_value = {"enabled": False}
        mock_session.request.return_value = mock_response

        result = client.get_say_detection_config()
        assert result["enabled"] is False

    def test_export_accounts(self, client, mock_session):
        """Test account export."""
        mock_response = Mock()
        mock_response.text = '{"accounts": []}'
        mock_response.json.return_value = {"accounts": []}
        mock_session.request.return_value = mock_response

        result = client.export_accounts()
        assert "accounts" in result

    def test_import_accounts(self, client, mock_session):
        """Test account import."""
        mock_response = Mock()
        mock_response.text = '{"imported": 1}'
        mock_response.json.return_value = {"imported": 1}
        mock_session.request.return_value = mock_response

        result = client.import_accounts([{"name": "test"}])
        assert result["imported"] == 1

    def test_gm_alerts(self, client, mock_session):
        """Test GM alert endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []
        mock_session.request.return_value = mock_response

        result = client.list_gm_alerts()
        assert result == []

    def test_timestamp_config(self, client, mock_session):
        """Test timestamp configuration endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []
        mock_session.request.return_value = mock_response

        result = client.list_timestamp_configs()
        assert result == []

    def test_auto_accept_settings(self, client, mock_session):
        """Test auto-accept settings endpoints."""
        mock_response = Mock()
        mock_response.text = '{"enabled": true}'
        mock_response.json.return_value = {"enabled": True}
        mock_session.request.return_value = mock_response

        result = client.get_auto_accept_settings()
        assert result["enabled"] is True

    def test_player_watch_config(self, client, mock_session):
        """Test player watch configuration endpoints."""
        mock_response = Mock()
        mock_response.text = '{"watch_list": []}'
        mock_response.json.return_value = {"watch_list": []}
        mock_session.request.return_value = mock_response

        result = client.get_player_watch_config()
        assert "watch_list" in result
