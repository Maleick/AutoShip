# Final Catalog

## Restored Findings

- Removed the release workflow's direct protected-branch push path by dropping `@semantic-release/git` from semantic-release configuration and CI package loading.
- Restored TextQuest policy auto-detection by treating `policyProfile: "default"` as the setup default, not as a hard override that disables repo detection.
- Restored installed policy asset lookup by falling back from target repo `policies/` to the packaged AutoShip `policies/` directory.
- Hardened monitor liveness detection by comparing both logical and physical workspace paths against a snapshot of `ps` output.

## Verified Survey Results

- No circular dependencies detected.
- No exact duplication detected by the configured threshold.
- No npm audit vulnerabilities detected.
- TextQuest read-only AutoShip plan executed successfully and found no eligible ready issues at the time of the run.

## Follow-Up Candidates

- Consider adding a `knip` config that excludes publish artifacts and plugin assets to reduce false positives in future archaeology surveys.
- Decide whether release commits/changelog updates should be handled by a separate release PR workflow rather than direct `main` commits.
