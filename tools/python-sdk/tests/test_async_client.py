"""Tests for the asynchronous TextQuest client."""

import pytest
from unittest.mock import AsyncMock, Mock, patch, MagicMock
from textquest_sdk import AsyncTextQuestClient


@pytest.mark.asyncio
class TestAsyncTextQuestClient:
    """Test cases for the AsyncTextQuestClient."""

    @pytest.fixture
    async def client(self):
        """Create an async client instance."""
        with patch('textquest_sdk.async_client.httpx.AsyncClient'):
            return AsyncTextQuestClient("http://localhost:3001")

    @pytest.mark.asyncio
    async def test_client_initialization(self):
        """Test async client initialization."""
        client = AsyncTextQuestClient("http://example.com", api_token="test-token")
        assert client.base_url == "http://example.com"
        assert client.api_token == "test-token"
        await client.close()

    @pytest.mark.asyncio
    async def test_health_endpoint(self, client):
        """Test async health check endpoint."""
        mock_response = Mock()
        mock_response.text = '{"status": "ok", "version": "0.1.0"}'
        mock_response.json.return_value = {"status": "ok", "version": "0.1.0"}

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.health()

            assert result["status"] == "ok"
            assert result["version"] == "0.1.0"

    @pytest.mark.asyncio
    async def test_list_sessions(self, client):
        """Test async listing sessions."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.list_sessions()

            assert result == []

    @pytest.mark.asyncio
    async def test_create_account(self, client):
        """Test async account creation."""
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

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.create_account(account_data)

            assert result["name"] == "test_account"

    @pytest.mark.asyncio
    async def test_list_accounts(self, client):
        """Test async listing accounts."""
        mock_response = Mock()
        mock_response.text = '[{"name": "account1"}]'
        mock_response.json.return_value = [{"name": "account1"}]

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.list_accounts()

            assert len(result) == 1
            assert result[0]["name"] == "account1"

    @pytest.mark.asyncio
    async def test_get_account(self, client):
        """Test async getting a specific account."""
        mock_response = Mock()
        mock_response.text = '{"name": "test_account"}'
        mock_response.json.return_value = {"name": "test_account"}

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.get_account("test_account")

            assert result["name"] == "test_account"

    @pytest.mark.asyncio
    async def test_update_account(self, client):
        """Test async updating an account."""
        update_data = {"group": 2}
        mock_response = Mock()
        mock_response.text = '{"name": "test_account", "group": 2}'
        mock_response.json.return_value = {"name": "test_account", "group": 2}

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.update_account("test_account", update_data)

            assert result["group"] == 2

    @pytest.mark.asyncio
    async def test_delete_account(self, client):
        """Test async deleting an account."""
        mock_response = AsyncMock()
        mock_response.raise_for_status = Mock()

        with patch('textquest_sdk.async_client.httpx.AsyncClient.delete', new_callable=AsyncMock) as mock_delete:
            mock_delete.return_value = mock_response

            await client.delete_account("test_account")

            mock_delete.assert_called_once()

    @pytest.mark.asyncio
    async def test_economy_settings(self, client):
        """Test async economy settings endpoints."""
        mock_response = Mock()
        mock_response.text = '{"krono_farming_enabled": true}'
        mock_response.json.return_value = {"krono_farming_enabled": True}

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.get_economy_settings()
            assert result["krono_farming_enabled"] is True

            await client.set_economy_settings({"krono_farming_enabled": False})
            assert mock_request.call_count == 2

    @pytest.mark.asyncio
    async def test_wealth_endpoint(self, client):
        """Test async wealth endpoint."""
        mock_response = Mock()
        mock_response.text = '{"total_platinum": 5000}'
        mock_response.json.return_value = {"total_platinum": 5000}

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.get_wealth()
            assert result["total_platinum"] == 5000

    @pytest.mark.asyncio
    async def test_soul_states(self, client):
        """Test async soul state endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.list_soul_states()
            assert result == []

    @pytest.mark.asyncio
    async def test_loot_rules(self, client):
        """Test async loot rules endpoints."""
        mock_response = Mock()
        mock_response.text = '{"auto_split_stackables": true}'
        mock_response.json.return_value = {"auto_split_stackables": True}

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.get_loot_rules()
            assert result["auto_split_stackables"] is True

    @pytest.mark.asyncio
    async def test_spawn_alerts(self, client):
        """Test async spawn alert endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.list_spawn_alerts()
            assert result == []

    @pytest.mark.asyncio
    async def test_character_configs(self, client):
        """Test async character configuration endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.list_character_configs()
            assert result == []

    @pytest.mark.asyncio
    async def test_alert_config(self, client):
        """Test async alert configuration endpoints."""
        mock_response = Mock()
        mock_response.text = '{"enabled": true}'
        mock_response.json.return_value = {"enabled": True}

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.get_alert_config()
            assert result["enabled"] is True

    @pytest.mark.asyncio
    async def test_chat_pattern_rules(self, client):
        """Test async chat pattern rules endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.list_chat_pattern_rules()
            assert result == []

    @pytest.mark.asyncio
    async def test_async_context_manager(self):
        """Test async context manager usage."""
        with patch('textquest_sdk.async_client.httpx.AsyncClient'):
            async with AsyncTextQuestClient("http://localhost:3001") as client:
                assert client is not None
                assert hasattr(client, 'api_token')

    @pytest.mark.asyncio
    async def test_url_normalization(self):
        """Test that trailing slashes are removed from base URL."""
        client = AsyncTextQuestClient("http://localhost:3001/")
        assert client.base_url == "http://localhost:3001"
        await client.close()

    @pytest.mark.asyncio
    async def test_export_accounts(self, client):
        """Test async account export."""
        mock_response = Mock()
        mock_response.text = '{"accounts": []}'
        mock_response.json.return_value = {"accounts": []}

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.export_accounts()
            assert "accounts" in result

    @pytest.mark.asyncio
    async def test_import_accounts(self, client):
        """Test async account import."""
        mock_response = Mock()
        mock_response.text = '{"imported": 1}'
        mock_response.json.return_value = {"imported": 1}

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.import_accounts([{"name": "test"}])
            assert result["imported"] == 1

    @pytest.mark.asyncio
    async def test_kill_tracker_operations(self, client):
        """Test async kill tracker endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.list_kill_tracker_records()
            assert result == []

    @pytest.mark.asyncio
    async def test_xassist_config(self, client):
        """Test async XAssist configuration endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.list_xassist_configs()
            assert result == []

    @pytest.mark.asyncio
    async def test_say_detection(self, client):
        """Test async say detection endpoints."""
        mock_response = Mock()
        mock_response.text = '{"enabled": false}'
        mock_response.json.return_value = {"enabled": False}

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.get_say_detection_config()
            assert result["enabled"] is False

    @pytest.mark.asyncio
    async def test_gm_alerts(self, client):
        """Test async GM alert endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.list_gm_alerts()
            assert result == []

    @pytest.mark.asyncio
    async def test_timestamp_config(self, client):
        """Test async timestamp configuration endpoints."""
        mock_response = Mock()
        mock_response.text = '[]'
        mock_response.json.return_value = []

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.list_timestamp_configs()
            assert result == []

    @pytest.mark.asyncio
    async def test_auto_accept_settings(self, client):
        """Test async auto-accept settings endpoints."""
        mock_response = Mock()
        mock_response.text = '{"enabled": true}'
        mock_response.json.return_value = {"enabled": True}

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.get_auto_accept_settings()
            assert result["enabled"] is True

    @pytest.mark.asyncio
    async def test_player_watch_config(self, client):
        """Test async player watch configuration endpoints."""
        mock_response = Mock()
        mock_response.text = '{"watch_list": []}'
        mock_response.json.return_value = {"watch_list": []}

        with patch('textquest_sdk.async_client.httpx.AsyncClient.request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_response

            result = await client.get_player_watch_config()
            assert "watch_list" in result
