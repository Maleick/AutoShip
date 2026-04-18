"""Shared utility functions for TextQuest SDK clients."""

from typing import Any, Dict, List


def summarize_kill_tracker_history(history: List[Dict[str, Any]]) -> Dict[str, Any]:
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
