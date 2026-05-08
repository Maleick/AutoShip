#!/bin/bash
# AutoResearch decomposer
set -euo pipefail
cd /home/kara/projects/AutoResearch || exit 1
npm run decompose -- --input /home/kara/projects/AutoShip/.autoship/hermes-plan.json --output /home/kara/projects/AutoShip/.autoship/autoresearch-decomposition.json 2>&1 || echo "AutoResearch decompose not available"
