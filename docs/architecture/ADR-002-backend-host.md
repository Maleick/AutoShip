# ADR-002: Backend Host Selection

**Status**: Accepted  
**Decision Date**: 2026-04-25  
**Related Issue**: #2252

---

## Context

TextQuest backend requires persistent hosting for orchestrator and game state (SQLite). Constraints:
- Cost-conscious (no significant cloud spend for development/testing phase)
- SQLite-based persistence (no managed DB required)
- Low-to-moderate traffic initially (1–20 concurrent clients)
- Elasticity requirement: can scale from single instance to multi-region
- Learning value: prefer providers with strong documentation and Rust ecosystem support

Previous research (#2247) evaluated Oracle Free Tier, DigitalOcean, Fly.io, Hetzner, and self-hosted Mac mini.

---

## Decision

**Primary Host: Oracle Cloud Free Tier**  
**Secondary Host: DigitalOcean (failover/fallback)**

### Oracle Free Tier
- **Always-free compute**: 4x ARM Ampere A1 vCPUs (3,000 OCPU hours/month) + 24 GB RAM
- **Cost**: $0/month (unlimited)
- **Storage**: 200 GB block storage (always-free), additional $0.0255/GB
- **Network**: 10 TB/month outbound data transfer (always-free)
- **Fit**: Sufficient for full TextQuest orchestrator + 20+ concurrent clients; no risk of bill shock
- **Trade-off**: ARM CPU (not x86); slightly steeper OCI learning curve vs DigitalOcean

### DigitalOcean Fallback
- **VM**: CX41 (8 vCPU, 16 GB RAM) @ $24/month or CX21 (4 vCPU, 8 GB RAM) @ $12/month
- **Cost**: Predictable, scalable
- **Fit**: If Oracle Free Tier experiences downtime or if team hits unexpected complexity with OCI
- **Trade-off**: Not free; requires spending approval

---

## Rationale

1. **Cost ceiling**: Oracle Free Tier eliminates financial risk and approval friction during development. DigitalOcean gives us an escape hatch if OCI proves incompatible.

2. **Capacity**: 24 GB RAM and 4 vCPUs easily handle orchestrator + 20+ concurrent EQ clients; headroom for growth without rearchitecting.

3. **Persistence**: Both platforms support block storage + snapshots for full point-in-time recovery. SQLite + WAL mode + daily backups to object storage meets durability requirements.

4. **Learning**: OCI's free tier has strong Rust/Docker support and excellent documentation; investment in OCI knowledge applies to enterprise cloud work.

5. **Fallback simplicity**: DigitalOcean integration is straightforward for migration if needed (both support Linux VMs, Docker, standard tooling).

---

## Alternatives Considered

### Self-hosted Mac mini
- **Cost**: ~$30/month (electricity + ISP)
- **Why not**: Not cloud-native; requires physical maintenance; not suitable for production (no redundancy, location constraint)

### Fly.io
- **Cost**: $0–10/month (free tier + minimal overages)
- **Why not**: Smaller ecosystem; no persistent block storage; better for stateless workloads

### Hetzner
- **Cost**: ~€3/month
- **Why not**: Good value but requires EU residency/company registration; less familiar to North American teams

### DigitalOcean Primary
- **Cost**: $12–24/month minimum
- **Why not**: Oracle Free Tier is zero-cost; DigitalOcean reserved for fallback

---

## Consequences

### Positive
- **Zero cost** during development (no CFO/finance approval needed)
- **ARM CPU**: Forces portable Rust code; improves cross-platform robustness
- **Scale-out ready**: OCI's compute/storage model aligns with multi-region deployment patterns
- **Backup-native**: Object Storage integrations well with existing orchestrator design

### Negative
- **ARM architecture**: Requires testing on ARM CI runner; some deps may lack ARM wheels (mitigated by Rust)
- **OCI learning curve**: Slightly steeper onboarding than DigitalOcean; offset by strong docs
- **Lock-in risk**: Heavy investment in OCI SDKs makes DigitalOcean migration costlier long-term (low risk: standard Linux VM)

### Migration Path
If Oracle Free Tier hits limits (unlikely during M7–M8), migrate to DigitalOcean:
1. Snapshot SQLite DB + backups to S3 (or Oracle Object Storage)
2. Restore to DigitalOcean CX41
3. Update DNS / orchestrator config (1–2 hour downtime acceptable for non-production)

---

## Validation Checklist

- [ ] Oracle Free Tier account created and orchestrator deployed
- [ ] SQLite + WAL mode enabled; daily backups scripted
- [ ] DNS / load-balancer config supports failover to DigitalOcean
- [ ] Deployment playbook documents both hosts
- [ ] ARM Rust build tested on CI (or equivalent)

---

## Next Steps

1. Create OCI account and configure Free Tier compute/storage (GCP/AWS sync if applicable)
2. Deploy orchestrator Docker image to Oracle A1 instance
3. Establish daily backup cron → Oracle Object Storage
4. Document failover procedure (DNS cutover, DB restore)
5. Merge to master; close #2252
