# Result: #2260 — DevOps: backend deploy pipeline (Docker + GHA → chosen host)

## Scope completed
- Added root `Dockerfile` to build a production image for `textquest-web`:
  - Multi-stage build compiles frontend assets and Rust backend binary.
  - Runtime image serves the SPA from `/app/web/dist`, sets `TEXTQUEST_DATA_DIR=/app`, and includes a container healthcheck.
- Added `.github/workflows/deploy-textquest-web.yml`:
  - Triggers on `push` to `master` and `v*` tags.
  - Builds and pushes container image to configurable registry using secrets.
  - Deploys to remote host over SSH using secrets for host credentials.
  - Polls health endpoint after deploy (`/health`, then `/api/health`) and performs rollback on failure.
- Documented rollback path inline in deployment workflow (`PREVIOUS_IMAGE` capture + revert-to-prior image).

## Verification
- Confirmed workflow and Docker assets were created/updated in the expected locations:
  - `Dockerfile`
  - `.github/workflows/deploy-textquest-web.yml`
- Health check wiring uses `/health` fallback + `/api/health` compatibility.

## Notes
- No code-path changes were made in `textquest-web` Rust sources.
- All requested production-facing concerns are implemented through infra and deployment automation.
