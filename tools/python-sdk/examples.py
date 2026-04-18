"""
Basic usage examples for the TextQuest Python SDK.

These examples demonstrate both synchronous and asynchronous patterns.
"""

import asyncio
from textquest_sdk import TextQuestClient, AsyncTextQuestClient


# ─── Synchronous Examples ──────────────────────────────────────────

def example_sync_basic():
    """Basic synchronous usage example."""
    client = TextQuestClient("http://localhost:3001")

    # Check API health
    health = client.health()
    print(f"API Status: {health['status']}")
    print(f"Version: {health['version']}")

    client.close()


def example_sync_accounts():
    """Example: Account management with sync client."""
    with TextQuestClient("http://localhost:3001") as client:
        # List all accounts
        accounts = client.list_accounts()
        print(f"Found {len(accounts)} accounts:")
        for account in accounts:
            print(f"  - {account['name']}: {account['character']} ({account['class']})")

        # Create a new account
        new_account = client.create_account({
            "name": "wizard1",
            "server": "Teek",
            "character": "Starfire",
            "class": "WIZ",
            "group": 1,
            "status": "active",
            "password": "secret123"
        })
        print(f"Created account: {new_account['name']}")

        # Update account
        client.update_account("wizard1", {"group": 2, "status": "paused"})
        print("Updated wizard1 to group 2 and paused")

        # Get specific account
        account = client.get_account("wizard1")
        print(f"Account details: {account}")


def example_sync_economy():
    """Example: Economy management with sync client."""
    with TextQuestClient("http://localhost:3001") as client:
        # Get economy settings
        settings = client.get_economy_settings()
        print(f"Krono farming enabled: {settings.get('krono_farming_enabled')}")

        # Update economy settings
        client.set_economy_settings({
            "krono_farming_enabled": True,
            "restock_interval_minutes": 45,
            "target_wealth_platinum": 15000
        })

        # Manage vendor routes
        routes = client.list_vendor_routes()
        print(f"Vendor routes: {len(routes)}")

        # Create new route
        route = client.create_vendor_route({
            "name": "Qeynos Hub",
            "vendors": ["Merchant1", "Merchant2"],
            "priority": 1
        })
        print(f"Created route: {route['name']}")

        # Get wealth info
        wealth = client.get_wealth()
        print(f"Total platinum: {wealth.get('total_platinum')}")


def example_sync_loot():
    """Example: Loot management with sync client."""
    with TextQuestClient("http://localhost:3001") as client:
        # Get loot rules
        rules = client.get_loot_rules()
        print(f"Auto-split stackables: {rules.get('auto_split_stackables')}")

        # Update loot rules
        client.set_loot_rules({
            "auto_split_stackables": True,
            "reserve_for_crafting": True,
            "master_looter_enabled": False
        })

        # Set character filters
        client.set_loot_filter("Starfire", {
            "include_patterns": ["*Spell*", "*Mana*"],
            "exclude_patterns": ["*trash*", "*junk*"]
        })

        # Get loot history
        history = client.get_loot_history()
        print(f"Loot history entries: {len(history)}")


def example_sync_alerts():
    """Example: Alert management with sync client."""
    with TextQuestClient("http://localhost:3001") as client:
        # Get alert config
        config = client.get_alert_config()
        print(f"Alerts enabled: {config.get('enabled')}")

        # Update alert config
        client.set_alert_config({
            "enabled": True,
            "alert_on_gm": True,
            "alert_on_rare_spawn": True,
            "discord_webhook": "https://discord.com/api/webhooks/YOUR_WEBHOOK"
        })

        # Check spawn alerts
        alerts = client.list_spawn_alerts()
        print(f"Active spawn alerts: {len(alerts)}")

        # Manage spawn watch list
        client.add_spawn_watch_pattern("*rare*")
        client.add_spawn_watch_pattern("*epic*")

        watch_list = client.get_spawn_watch_list()
        print(f"Watch list: {watch_list}")


def example_sync_chat_patterns():
    """Example: Chat pattern rules with sync client."""
    with TextQuestClient("http://localhost:3001") as client:
        # List all rules
        rules = client.list_chat_pattern_rules()
        print(f"Total rules: {len(rules)}")

        # Create a rule
        rule = client.create_chat_pattern_rule({
            "pattern": r"sell (\w+)",
            "action": "execute_sell",
            "enabled": True,
            "cooldown_seconds": 30
        })
        print(f"Created rule: {rule['id']}")

        # Toggle rule
        client.toggle_chat_pattern_rule(rule['id'], False)

        # Get stats
        stats = client.get_chat_pattern_rules_stats()
        print(f"Rules stats: {stats}")


# ─── Asynchronous Examples ─────────────────────────────────────────

async def example_async_basic():
    """Basic asynchronous usage example."""
    async with AsyncTextQuestClient("http://localhost:3001") as client:
        # Check API health
        health = await client.health()
        print(f"API Status: {health['status']}")
        print(f"Version: {health['version']}")


async def example_async_accounts():
    """Example: Account management with async client."""
    async with AsyncTextQuestClient("http://localhost:3001") as client:
        # List all accounts
        accounts = await client.list_accounts()
        print(f"Found {len(accounts)} accounts")

        # Create account
        new_account = await client.create_account({
            "name": "cleric1",
            "server": "Teek",
            "character": "Mercy",
            "class": "CLR",
            "group": 1,
            "status": "active",
            "password": "secret456"
        })
        print(f"Created account: {new_account['name']}")

        # Get account
        account = await client.get_account("cleric1")
        print(f"Account: {account}")


async def example_async_concurrent():
    """Example: Concurrent async operations."""
    async with AsyncTextQuestClient("http://localhost:3001") as client:
        # Run multiple operations concurrently
        health, accounts, settings = await asyncio.gather(
            client.health(),
            client.list_accounts(),
            client.get_economy_settings()
        )

        print(f"API Health: {health['status']}")
        print(f"Number of accounts: {len(accounts)}")
        print(f"Economy settings: {settings}")


async def example_async_soul_monitoring():
    """Example: Monitor soul/character states."""
    async with AsyncTextQuestClient("http://localhost:3001") as client:
        # Get all soul states
        souls = await client.list_soul_states()
        print(f"Active characters: {len(souls)}")

        for soul in souls:
            print(f"  - {soul.get('character_name')}: {soul.get('zone')}")

        # Get specific character state
        if souls:
            char_id = souls[0]['character_id']
            state = await client.get_soul_state(char_id)
            print(f"Character state: {state}")

        # Get audit trail
        audit = await client.get_soul_audit()
        print(f"Audit entries: {len(audit)}")


async def example_async_batch_operations():
    """Example: Batch update operations."""
    async with AsyncTextQuestClient("http://localhost:3001") as client:
        # Get all characters and update their configs
        configs = await client.list_character_configs()

        for config in configs:
            char = config.get('character')
            # Update XAssist config
            await client.set_xassist_config(char, {
                "enabled": True,
                "assist_target": "MainLeader",
                "assist_range": 100
            })
            print(f"Updated XAssist for {char}")


async def example_async_export_import():
    """Example: Export and import accounts."""
    async with AsyncTextQuestClient("http://localhost:3001") as client:
        # Export accounts
        exported = await client.export_accounts()
        print(f"Exported {exported['imported']} accounts")

        # Save to file
        import json
        with open("accounts_backup.json", "w") as f:
            json.dump(exported["accounts"], f)

        # Later: import from file
        with open("accounts_backup.json", "r") as f:
            accounts = json.load(f)

        result = await client.import_accounts(accounts)
        print(f"Imported {result['imported']} accounts")


# ─── Main Runner ───────────────────────────────────────────────────

def main_sync():
    """Run synchronous examples."""
    print("=" * 60)
    print("TextQuest SDK - Synchronous Examples")
    print("=" * 60)

    print("\n1. Basic API Check")
    print("-" * 60)
    example_sync_basic()

    print("\n2. Account Management")
    print("-" * 60)
    # example_sync_accounts()  # Uncomment to run (requires running API)

    print("\n3. Economy Management")
    print("-" * 60)
    # example_sync_economy()  # Uncomment to run

    print("\n4. Loot Management")
    print("-" * 60)
    # example_sync_loot()  # Uncomment to run

    print("\n5. Alert Management")
    print("-" * 60)
    # example_sync_alerts()  # Uncomment to run

    print("\n6. Chat Pattern Rules")
    print("-" * 60)
    # example_sync_chat_patterns()  # Uncomment to run


async def main_async():
    """Run asynchronous examples."""
    print("\n" + "=" * 60)
    print("TextQuest SDK - Asynchronous Examples")
    print("=" * 60)

    print("\n1. Basic API Check (Async)")
    print("-" * 60)
    await example_async_basic()

    print("\n2. Concurrent Operations")
    print("-" * 60)
    # await example_async_concurrent()  # Uncomment to run

    print("\n3. Soul Monitoring")
    print("-" * 60)
    # await example_async_soul_monitoring()  # Uncomment to run

    print("\n4. Batch Operations")
    print("-" * 60)
    # await example_async_batch_operations()  # Uncomment to run


if __name__ == "__main__":
    # Run sync examples
    main_sync()

    # Run async examples
    asyncio.run(main_async())
