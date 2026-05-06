#!/usr/bin/env python3
"""
AutoShip intelligent model router.
Analyzes issue title + labels to determine task complexity and selects optimal model.
"""
import json
import sys
from pathlib import Path

AUTOSHIP_ROOT = Path(__file__).parent.parent.parent
USAGE_LOG = AUTOSHIP_ROOT / ".autoship" / "usage-log.json"


def routing_config_path():
    runtime_config = AUTOSHIP_ROOT / ".autoship" / "model-routing.json"
    if runtime_config.exists():
        return runtime_config
    return AUTOSHIP_ROOT / "config" / "model-routing.json"


def load_routing_config():
    with open(routing_config_path()) as f:
        return json.load(f)


def load_usage_log():
    if USAGE_LOG.exists():
        with open(USAGE_LOG) as f:
            return json.load(f)
    return {"last_model": "", "tier_usage": {}, "task_type_usage": {}}


def save_usage_log(data):
    USAGE_LOG.parent.mkdir(parents=True, exist_ok=True)
    with open(USAGE_LOG, "w") as f:
        json.dump(data, f, indent=2)


def analyze_task(title: str, labels: list) -> dict:
    """Analyze issue to determine complexity, domain, and optimal model tier."""
    title_lower = title.lower()
    labels_lower = [label.lower() for label in labels]

    complexity_score = 0
    complexity_indicators = {
        "simple": ["audit", "remove", "delete", "cleanup", "prune", "fix typo", "format"],
        "medium": ["implement", "add", "create", "update", "refactor", "parity"],
        "complex": ["architect", "redesign", "orchestrate", "mq2spawns", "mq2autosize", "easyfind", "packet hook", "etw", "page encrypt"],
    }

    for word in complexity_indicators["simple"]:
        if word in title_lower:
            complexity_score -= 1
    for word in complexity_indicators["medium"]:
        if word in title_lower:
            complexity_score += 1
    for word in complexity_indicators["complex"]:
        if word in title_lower:
            complexity_score += 2

    domain = "general"
    if any("domain:combat" in label for label in labels_lower):
        domain = "combat"
    elif any("domain:nav" in label for label in labels_lower):
        domain = "navigation"
    elif any("domain:infra" in label for label in labels_lower):
        domain = "infrastructure"

    if "audit" in title_lower or "dead_code" in title_lower:
        task_type = "audit"
    elif "implement" in title_lower and "parity" in title_lower:
        task_type = "parity"
    elif "implement" in title_lower:
        task_type = "implementation"
    elif "fix" in title_lower:
        task_type = "fix"
    elif "docs" in title_lower or "readme" in title_lower:
        task_type = "docs"
    else:
        task_type = "general"

    if complexity_score >= 2 or domain in ["combat", "navigation"] or task_type == "parity":
        recommended_tier = "go_paid"
        recommended_model = "opencode-go/deepseek-v4-pro"
    elif complexity_score <= -1 or task_type == "audit":
        recommended_tier = "zen_free"
        recommended_model = "opencode-zen/gpt-5-nano"
    else:
        recommended_tier = "zen_free"
        recommended_model = "opencode-zen/nemotron-3-super-free"

    return {
        "complexity_score": complexity_score,
        "domain": domain,
        "task_type": task_type,
        "recommended_tier": recommended_tier,
        "recommended_model": recommended_model,
    }


def default_fallback(config: dict) -> str:
    fallback = config.get("defaultFallback") or config.get("default_fallback")
    if isinstance(fallback, str) and fallback:
        return fallback

    for tier in config.get("tiers", []):
        for model in tier.get("models", []):
            model_id = model.get("id")
            if isinstance(model_id, str) and model_id:
                return model_id

    return ""


def get_model_from_tier(tier_name: str, config: dict, usage: dict, task_type: str = "") -> str:
    """Get next model from tier with round-robin and task-aware selection."""
    tiers = config.get("tiers", [])
    tier = next((candidate for candidate in tiers if candidate["name"] == tier_name), None)

    if not tier:
        return default_fallback(config)

    models = tier.get("models", [])
    if not models:
        return default_fallback(config)

    model_ids = [model["id"] for model in models]

    if task_type:
        task_capable = []
        for model in models:
            caps = model.get("capabilities", [])
            if task_type in caps or "code" in caps:
                task_capable.append(model["id"])
        if task_capable:
            model_ids = task_capable

    tier_usage = usage.get("tier_usage", {}).get(tier_name, [])
    for model_id in model_ids:
        if model_id not in tier_usage[-3:]:
            return model_id

    return model_ids[0]


def dispatch_with_routing(title: str = "", labels=None, task_type: str = "code", complexity: str = "simple") -> str:
    """Main dispatch function - intelligently route based on task analysis."""
    config = load_routing_config()
    usage = load_usage_log()
    labels = labels or []

    analysis = analyze_task(title, labels)
    recommended_tier = analysis["recommended_tier"]
    recommended_model = analysis["recommended_model"]

    model = get_model_from_tier(recommended_tier, config, usage, analysis["task_type"])

    tier_models = next((tier.get("models", []) for tier in config.get("tiers", []) if tier["name"] == recommended_tier), [])
    if any(candidate["id"] == recommended_model for candidate in tier_models):
        model = recommended_model

    usage["last_model"] = model
    usage.setdefault("tier_usage", {}).setdefault(recommended_tier, []).append(model)
    usage.setdefault("task_type_usage", {}).setdefault(analysis["task_type"], []).append(model)
    save_usage_log(usage)

    log_dir = AUTOSHIP_ROOT / ".autoship" / "logs"
    log_dir.mkdir(parents=True, exist_ok=True)
    with open(log_dir / "model-selection.log", "a") as f:
        f.write(f"{model} | tier={recommended_tier} | task={analysis['task_type']} | domain={analysis['domain']} | complexity={analysis['complexity_score']} | title={title[:60]}\n")

    return model


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: model-router.py <title> [labels_json] [task_type] [complexity]")
        sys.exit(1)

    title_arg = sys.argv[1]
    labels_arg = json.loads(sys.argv[2]) if len(sys.argv) > 2 else []
    task_type_arg = sys.argv[3] if len(sys.argv) > 3 else "code"
    complexity_arg = sys.argv[4] if len(sys.argv) > 4 else "simple"

    print(dispatch_with_routing(title_arg, labels_arg, task_type_arg, complexity_arg))
