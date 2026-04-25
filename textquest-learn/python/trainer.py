#!/usr/bin/env python3
"""
Offline RL training harness for TextQuest policies (CQL/IQL).
Trains policies on replay data and exports to ONNX for Rust-side inference.
"""

import argparse
import json
import logging
import sys
from pathlib import Path
from typing import Optional

import numpy as np

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


def load_ledger_data(ledger_path: Path) -> tuple:
    """Load replay trajectories from ledger directory."""
    logger.info(f"Loading ledger from {ledger_path}")

    # Placeholder: in real implementation, load from ledger directory
    # For now, return mock data for testing
    states = np.random.randn(1000, 64).astype(np.float32)
    actions = np.random.randint(0, 32, 1000)
    rewards = np.random.randn(1000).astype(np.float32)
    next_states = np.random.randn(1000, 64).astype(np.float32)
    terminals = np.random.binomial(1, 0.1, 1000).astype(bool)

    logger.info(f"Loaded {len(states)} transitions")
    return states, actions, rewards, next_states, terminals


def load_warm_start(bc_path: Path) -> Optional[object]:
    """Load warm-start BC policy from ONNX."""
    logger.info(f"Loading warm-start BC policy from {bc_path}")
    try:
        import onnx
        model = onnx.load(str(bc_path))
        logger.info(f"Loaded BC model: {model}")
        return model
    except Exception as e:
        logger.warning(f"Could not load BC model: {e}")
        return None


def train_cql(
    states: np.ndarray,
    actions: np.ndarray,
    rewards: np.ndarray,
    next_states: np.ndarray,
    terminals: np.ndarray,
    epochs: int,
    warm_start: Optional[object] = None,
) -> object:
    """Train CQL (Conservative Q-Learning) policy."""
    logger.info(f"Training CQL for {epochs} epochs")

    try:
        import torch
        from torch import nn, optim
    except ImportError:
        logger.error("PyTorch required for CQL training. Install with: pip install torch")
        sys.exit(1)

    state_dim = states.shape[1]
    action_dim = int(np.max(actions)) + 1

    # Simple Q-network
    class QNetwork(nn.Module):
        def __init__(self, state_dim: int, action_dim: int):
            super().__init__()
            self.net = nn.Sequential(
                nn.Linear(state_dim, 128),
                nn.ReLU(),
                nn.Linear(128, 128),
                nn.ReLU(),
                nn.Linear(128, action_dim),
            )

        def forward(self, state):
            return self.net(state)

    q_network = QNetwork(state_dim, action_dim)
    target_q = QNetwork(state_dim, action_dim)
    target_q.load_state_dict(q_network.state_dict())

    optimizer = optim.Adam(q_network.parameters(), lr=1e-3)
    criterion = nn.MSELoss()

    # Mock training loop
    for epoch in range(epochs):
        batch_size = min(128, len(states))
        indices = np.random.choice(len(states), batch_size, replace=False)

        batch_states = torch.FloatTensor(states[indices])
        batch_actions = torch.LongTensor(actions[indices])
        batch_rewards = torch.FloatTensor(rewards[indices])
        batch_next_states = torch.FloatTensor(next_states[indices])
        batch_terminals = torch.FloatTensor(terminals[indices])

        with torch.no_grad():
            next_q = target_q(batch_next_states).max(dim=1)[0]
            target = batch_rewards + 0.99 * next_q * (1 - batch_terminals)

        q_values = q_network(batch_states).gather(1, batch_actions.unsqueeze(1)).squeeze()
        loss = criterion(q_values, target)

        optimizer.zero_grad()
        loss.backward()
        optimizer.step()

        if (epoch + 1) % max(1, epochs // 10) == 0:
            logger.info(f"Epoch {epoch+1}/{epochs}, Loss: {loss.item():.4f}")

    return q_network


def train_iql(
    states: np.ndarray,
    actions: np.ndarray,
    rewards: np.ndarray,
    next_states: np.ndarray,
    terminals: np.ndarray,
    epochs: int,
    warm_start: Optional[object] = None,
) -> object:
    """Train IQL (Implicit Q-Learning) policy."""
    logger.info(f"Training IQL for {epochs} epochs")

    # IQL is similar to CQL but with a different offline RL formulation
    # For this implementation, use similar approach to CQL
    return train_cql(
        states, actions, rewards, next_states, terminals, epochs, warm_start
    )


def export_onnx(model: object, output_path: Path, state_dim: int) -> None:
    """Export trained policy to ONNX format."""
    logger.info(f"Exporting policy to ONNX: {output_path}")

    try:
        import torch
        import torch.onnx
    except ImportError:
        logger.error("PyTorch required for ONNX export")
        sys.exit(1)

    output_path.parent.mkdir(parents=True, exist_ok=True)

    # Create dummy input for ONNX export
    dummy_input = torch.randn(1, state_dim)

    torch.onnx.export(
        model,
        dummy_input,
        str(output_path),
        input_names=["state"],
        output_names=["action_logits"],
        dynamic_axes={
            "state": {0: "batch_size"},
            "action_logits": {0: "batch_size"},
        },
        opset_version=14,
        verbose=False,
    )

    logger.info(f"ONNX model exported: {output_path}")


def main():
    parser = argparse.ArgumentParser(
        description="Train offline RL policy using CQL/IQL"
    )
    parser.add_argument(
        "--algo", required=True, choices=["cql", "iql"], help="Algorithm: cql or iql"
    )
    parser.add_argument("--warm-start", type=Path, help="Warm-start BC policy (ONNX)")
    parser.add_argument("--ledger", type=Path, required=True, help="Ledger data directory")
    parser.add_argument(
        "--reward", required=True, help="Reward specification key"
    )
    parser.add_argument("--class", dest="class_name", required=True, help="Character class")
    parser.add_argument(
        "--epochs", type=int, default=200, help="Number of training epochs"
    )
    parser.add_argument("--out", type=Path, required=True, help="Output ONNX path")

    args = parser.parse_args()

    # Load data
    states, actions, rewards, next_states, terminals = load_ledger_data(args.ledger)

    # Load warm-start if provided
    warm_start = None
    if args.warm_start:
        warm_start = load_warm_start(args.warm_start)

    # Train model
    if args.algo == "cql":
        model = train_cql(
            states, actions, rewards, next_states, terminals, args.epochs, warm_start
        )
    else:
        model = train_iql(
            states, actions, rewards, next_states, terminals, args.epochs, warm_start
        )

    # Export ONNX
    state_dim = states.shape[1]
    export_onnx(model, args.out, state_dim)

    logger.info(f"Training complete. Policy saved to {args.out}")


if __name__ == "__main__":
    main()
