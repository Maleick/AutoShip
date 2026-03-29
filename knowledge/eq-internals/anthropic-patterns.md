# Anthropic Long-Running App Patterns — Applied to Frostreaver

Source: https://www.anthropic.com/engineering/harness-design-long-running-apps

## Patterns We Already Use
- **Structured artifact handoffs**: HANDOFF.md carries state between sessions
- **Sprint-based decomposition**: M1-M8 milestones with clear boundaries
- **Separated evaluation**: Peer review skill (GPT + Gemini as independent reviewers)
- **Version control integration**: Git-based change tracking
- **Multi-agent specialization**: TeamCreate with role-specific agents

## Patterns to Adopt
1. **Context resets over compaction**: Start fresh agents with clean context + HANDOFF rather than letting context grow indefinitely. Better than summarization.
2. **Sprint contracts**: Before each task, define testable "done" criteria (not just "it compiles")
3. **Active testing**: Use Windows MCP to interact with running EQ client (click through, verify state) rather than just reading logs
4. **Evaluator calibration**: Add few-shot examples to peer review prompts showing what "good" looks like for this codebase
5. **Cost tracking per phase**: Track token usage across research vs implementation vs review
6. **Harness simplification**: Re-examine automation assumptions with each model upgrade — what used to need scaffolding may now be handled by the model directly

## Key Insight
"Every component in a harness encodes an assumption about what the model can't do on its own."
→ Periodically audit our automation (login chain, combat FSM, etc.) for over-engineering.
