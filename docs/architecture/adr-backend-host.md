# Backend Hosting Comparison: Architecture Decision Record

## Overview

This document evaluates five candidate hosting platforms for the TextQuest backend (decoupled `textquest-web` server). The workload is light (~5-10 concurrent EQ box sessions, ~1GB RAM total, synchronous API calls to Frostreaver).

## Comparison Matrix

| Dimension | Mac mini (Home) | DigitalOcean | Fly.io | Hetzner Cloud | Oracle Free Tier |
|-----------|-----------------|--------------|--------|---------------|------------------|
| **Monthly Cost** | ~$30 | $12–24 | $10–15 | €2.90–5.83 (~$3–6) | $0 (always free) |
| **RAM (Typical)** | 16GB | 2–4GB | 1–2GB | 2–4GB | 12GB |
| **CPU Cores** | 8 (M2) | 2–4 vCPU | Shared-1x | 2–4 vCPU | 2 ARM vCPU |
| **SLA / Uptime** | Home network (~95%) | 99.99% | 99.99% | 99.9% | 99.95% |
| **Persistent Storage** | 512GB+ SSD | $0.10/GB/mo | $0.15/GB/mo | $0.05/GB/mo | 200GB included |
| **Latency to Frostreaver** | <50ms (co-located) | ~20–40ms (NYC3) | ~20–40ms (US East) | ~80–120ms (Europe) | ~20–40ms (US) |
| **Deployment** | systemd / bare metal | Linux / Docker + systemd | Docker + Fly CLI | Linux / Docker / API | OCI CLI + systemd |
| **Vendor Lock-in** | None | Low | Medium | Low | High |
| **Exit Cost / Effort** | Immediate | Minutes (export) | Moderate (rebuild) | Days (API export) | Moderate (OCI config) |
| **Scaling Ceiling** | Vertical only (16GB RAM max) | Vertical ($192/mo for 32GB) | Pay-as-you-go (unbounded) | Vertical up to 192GB | Limited to free tier |
| **SQLite Persistence** | Direct file I/O | Block storage (reliable) | Volumes (S3-backed) | Block storage (reliable) | Block storage (reliable) |

## Cost Projections

### Low Traffic (1–2 concurrent sessions, ~200MB RAM)

| Provider | Monthly | Notes |
|----------|---------|-------|
| Mac mini | $30 | One-time $2500 capex; electricity + ISP |
| DigitalOcean | $12 | 2GB, 2vCPU (CX11 equivalent) |
| Fly.io | $8–10 | Free tier may suffice; minimal overages |
| Hetzner | €2.90 (~$3) | CX11: 1vCPU, 1GB |
| Oracle Free | $0 | Always-free tier sufficient |

### Medium Traffic (5–10 concurrent sessions, ~1GB RAM)

| Provider | Monthly | Notes |
|----------|---------|-------|
| Mac mini | $30 | Static; no scaling needed |
| DigitalOcean | $24 | 4GB, 4vCPU; room for growth |
| Fly.io | $12–18 | Compute + storage at $0.15/GB |
| Hetzner | €5.83 (~$6) | CX21: 2vCPU, 4GB |
| Oracle Free | $0 | 12GB RAM, 2vCPU always available |

### High Traffic (20+ concurrent sessions, ~3GB RAM)

| Provider | Monthly | Notes |
|----------|---------|-------|
| Mac mini | $30 | Cannot scale; must upgrade hardware |
| DigitalOcean | $48–96 | Upgrade to 8GB, 4vCPU+ |
| Fly.io | $30–50 | Scales with demand; no hard ceiling |
| Hetzner | €11.67+ (~$12) | CX31: 2vCPU, 8GB (no higher in CX line) |
| Oracle Free | $0 | Still free; constraint is compute cores, not cost |

## Recommendation

**For TextQuest, recommend Oracle Free Tier as primary, with DigitalOcean as secondary.**

### Reasoning

1. **Oracle Free Tier** (First Choice)
   - **Why:** 12GB RAM + 200GB persistent storage, permanently free. No risk of surprise bills.
   - **Best for:** Current workload (5–10 sessions, ~1GB RAM) and reasonable growth margin (2–3x).
   - **Trade-off:** ARM-based CPU (Ampere); most Docker images support ARM, but niche libraries may require rebuilds. Deploy via OCI CLI; slightly steeper learning curve than Docker-only platforms.
   - **Exit risk:** Moderate — OCI-specific configs (IAM, networking) require porting.

2. **DigitalOcean** (Fallback)
   - **Why:** If Oracle Free Tier is oversubscribed or encounter ARM compatibility issues, DO offers reliable, predictable pricing. NYC3 region has excellent latency to Frostreaver.
   - **Best for:** Scaling to 20–30+ concurrent sessions; lowest lock-in if exit needed.
   - **Cost:** $12–24/mo for 2–4GB; best per-dollar value in industry (minus Oracle).

### Why Not the Others

- **Mac mini:** One-time capex is high; scaling is vertical only. Good for dev/test but operationally risky (home network uptime, single point of failure).
- **Fly.io:** Attractive free tier, but "always-on" costs exceed DigitalOcean at medium scale. Fly-specific deployment (`fly.toml`, managed Postgres) increases lock-in.
- **Hetzner:** Europe-based; 80–120ms latency to Frostreaver (eastern US) is slower. Cost competitive but no free tier; 99.9% SLA is lower than alternatives.

## SQLite Persistence Strategy

Regardless of host:

1. **Daily backups** to object storage (S3, Oracle Cloud Storage, DigitalOcean Spaces).
2. **WAL mode** (write-ahead logging) enabled in SQLite for durability.
3. **Read replicas optional** if higher availability needed; single-writer architecture for now.

For Oracle Free Tier: use block storage; for DO: use snapshots. Both support full point-in-time recovery.

## Next Steps

This is a research ADR. The recommendation above is **not final** — the project team should discuss the Oracle ARM CPU compatibility and OCI learning curve before committing. A separate issue should drive the final decision.

### Validation Checklist

- [ ] Determine if ARM CPU is acceptable for TextQuest stack (Docker, Python/Rust, SQLite).
- [ ] Test Oracle Free Tier deployment with current backend code.
- [ ] Measure actual latency from Oracle (Ashburn, Virginia) to Frostreaver.
- [ ] Verify SQLite backup/restore workflow with chosen platform.
