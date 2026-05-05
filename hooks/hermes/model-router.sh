#!/usr/bin/env python3
"""
AutoShip intelligent model router.
Analyzes issue title + labels to determine task complexity and selects optimal model.
"""
import json
import sys
import re
import os
from pathlib import Path

AUTOSHIP_ROOT = Path(__file__).parent.parent.parent
ROUTING_CONFIG = AUTOSHIP_ROOT / "config" / "model-routing.json"
USAGE_LOG = AUTOSHIP_ROOT / ".autoship" / "usage-log.json"

def load_routing_config():
    with open(ROUTING_CONFIG) as f:
        return json.load(f)

def load_usage_log():
    if USAGE_LOG.exists():
        with open(USAGE_LOG) as f:
            return json.load(f)
    return {"last_model": "", "tier_usage": {}, "task_type_usage": {}}

def save_usage_log(data):
    USAGE_LOG.parent.mkdir(parents=True, exist_ok=True)
    with open(USAGE_LOG, 'w') as f:
        json.dump(data, f, indent=2)

def analyze_task(title: str, labels: list) -> dict:
    """Analyze issue to determine complexity, domain, and optimal model tier."""
    
    title_lower = title.lower()
    labels_lower = [l.lower() for l in labels]
    
    # Complexity scoring
    complexity_score = 0
    complexity_indicators = {
        'simple': ['audit', 'remove', 'delete', 'cleanup', 'prune', 'fix typo', 'format'],
        'medium': ['implement', 'add', 'create', 'update', 'refactor', 'parity'],
        'complex': ['architect', 'redesign', 'orchestrate', 'mq2spawns', 'mq2autosize', 'easyfind', 'packet hook', 'etw', 'page encrypt']
    }
    
    for word in complexity_indicators['simple']:
        if word in title_lower:
            complexity_score -= 1
    for word in complexity_indicators['medium']:
        if word in title_lower:
            complexity_score += 1
    for word in complexity_indicators['complex']:
        if word in title_lower:
            complexity_score += 2
    
    # Domain detection
    domain = 'general'
    if any('domain:combat' in l for l in labels_lower):
        domain = 'combat'
    elif any('domain:nav' in l for l in labels_lower):
        domain = 'navigation'
    elif any('domain:infra' in l for l in labels_lower):
        domain = 'infrastructure'
    
    # Task type classification
    if 'audit' in title_lower or 'dead_code' in title_lower:
        task_type = 'audit'
    elif 'implement' in title_lower and 'parity' in title_lower:
        task_type = 'parity'
    elif 'implement' in title_lower:
        task_type = 'implementation'
    elif 'fix' in title_lower:
        task_type = 'fix'
    elif 'docs' in title_lower or 'readme' in title_lower:
        task_type = 'docs'
    else:
        task_type = 'general'
    
    # Determine tier based on complexity + domain + task type
    if complexity_score >= 2 or domain in ['combat', 'navigation'] or task_type == 'parity':
        recommended_tier = 'go_paid'
        recommended_model = 'opencode-go/deepseek-v4-pro'
    elif complexity_score <= -1 or task_type == 'audit':
        recommended_tier = 'zen_free'
        recommended_model = 'opencode-zen/gpt-5-nano'  # Fast for simple tasks
    else:
        recommended_tier = 'zen_free'
        recommended_model = 'opencode-zen/nemotron-3-super-free'  # Balanced
    
    return {
        'complexity_score': complexity_score,
        'domain': domain,
        'task_type': task_type,
        'recommended_tier': recommended_tier,
        'recommended_model': recommended_model
    }

def get_model_from_tier(tier_name: str, config: dict, usage: dict, task_type: str = '') -> str:
    """Get next model from tier with round-robin and task-aware selection."""
    
    tiers = config.get('tiers', [])
    tier = next((t for t in tiers if t['name'] == tier_name), None)
    
    if not tier:
        return 'kimi-k2.6'
    
    models = tier.get('models', [])
    if not models:
        return 'kimi-k2.6'
    
    model_ids = [m['id'] for m in models]
    
    # Task-aware selection: prefer models with matching capabilities
    if task_type:
        task_capable = []
        for m in models:
            caps = m.get('capabilities', [])
            if task_type in caps or 'code' in caps:
                task_capable.append(m['id'])
        if task_capable:
            model_ids = task_capable
    
    # Round-robin within filtered models
    last_model = usage.get('last_model', '')
    tier_usage = usage.get('tier_usage', {}).get(tier_name, [])
    
    # Find next model not recently used
    for model_id in model_ids:
        if model_id not in tier_usage[-3:]:  # Avoid last 3 used
            return model_id
    
    # Fallback to first
    return model_ids[0]

def dispatch_with_routing(title: str = '', labels: list = None, task_type: str = 'code', complexity: str = 'simple') -> str:
    """Main dispatch function - intelligently route based on task analysis."""
    
    config = load_routing_config()
    usage = load_usage_log()
    labels = labels or []
    
    # Analyze the task
    analysis = analyze_task(title, labels)
    recommended_tier = analysis['recommended_tier']
    recommended_model = analysis['recommended_model']
    
    # Try recommended tier first
    model = get_model_from_tier(recommended_tier, config, usage, analysis['task_type'])
    
    # If recommended model matches, use it
    if recommended_model in [m['id'] for m in config.get('tiers', [{}])[0].get('models', [])]:
        # Check if it's the specific model we want
        tier_models = next((t.get('models', []) for t in config.get('tiers', []) if t['name'] == recommended_tier), [])
        if any(m['id'] == recommended_model for m in tier_models):
            model = recommended_model
    
    # Update usage log
    usage['last_model'] = model
    usage.setdefault('tier_usage', {}).setdefault(recommended_tier, []).append(model)
    usage.setdefault('task_type_usage', {}).setdefault(analysis['task_type'], []).append(model)
    save_usage_log(usage)
    
    # Log selection
    log_dir = AUTOSHIP_ROOT / '.autoship' / 'logs'
    log_dir.mkdir(parents=True, exist_ok=True)
    with open(log_dir / 'model-selection.log', 'a') as f:
        f.write(f"{model} | tier={recommended_tier} | task={analysis['task_type']} | domain={analysis['domain']} | complexity={analysis['complexity_score']} | title={title[:60]}\n")
    
    return model

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: model-router.py <title> [labels_json] [task_type] [complexity]")
        sys.exit(1)
    
    title = sys.argv[1]
    labels = json.loads(sys.argv[2]) if len(sys.argv) > 2 else []
    task_type = sys.argv[3] if len(sys.argv) > 3 else 'code'
    complexity = sys.argv[4] if len(sys.argv) > 4 else 'simple'
    
    model = dispatch_with_routing(title, labels, task_type, complexity)
    print(model)

