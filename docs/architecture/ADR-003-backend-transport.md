# ADR-003: Backend Transport Mechanism

**Status:** Accepted  
**Issue:** #2253  
**Decision Date:** 2026-04-25

## Context

The TextQuest system requires reliable, secure bidirectional communication between:
- **textquest-dll**: Client-side game automation plugin (Windows)
- **textquest-web**: Backend orchestrator (Linux)

Transport must support:
- High-frequency updates (6-second tick alignment, sub-100ms latency)
- Automatic reconnection and state recovery
- Production-grade TLS encryption
- Firewall traversal (ISP/corporate blocking)
- Minimal operational overhead
- Horizontal scaling for 20+ concurrent clients

## Decision

**Choose: WebSocket over TLS (WSS)**

### Rationale

1. **Production-grade encryption**: TLS 1.3 provides cryptographic guarantees without custom protocol overhead. Certificate management via Let's Encrypt (free, automated).

2. **Firewall traversal**: WSS runs on standard HTTPS port 443, compatible with nearly all corporate/ISP firewalls. No special VPN or kernel-level setup required.

3. **Bidirectional semantics**: Native WebSocket support for server → client push. Eliminates polling overhead; clients receive state updates in real-time (<100ms over terrestrial links).

4. **Simplicity**: Rust ecosystem (tokio-tungstenite, tungstenite-rs) provides robust, audited implementations. Minimal custom code; leverages HTTP/1.1 upgrade semantics.

5. **Scaling**: WSS scales horizontally with standard load balancers (Nginx, HAProxy). No state pinning required; reconnection logic handles client → server2 failover.

6. **Debugging**: Standard HTTP/TLS tooling (curl, Wireshark, tcpdump) simplifies troubleshooting. No custom protocol analyzer needed.

## Alternatives Considered

### 1. Tailscale Mesh VPN
- **Pros**: Zero-config, peer-to-peer, excellent UX for ops
- **Cons**: Requires Tailscale client on all machines; adds 15–20 MB per installation; dependency on Tailscale's control plane; overkill for client↔server communication
- **Verdict**: Rejected as primary; recommended for **ops convenience** only (e.g., SSH access to backend during development)

### 2. WireGuard Direct
- **Pros**: Minimal overhead, kernel-native on Linux, proven security
- **Cons**: Requires kernel module on Windows; complex key management (36+ clients); firewall traversal requires port forwarding; no automatic failover
- **Verdict**: Rejected; appropriate only for machine-to-machine infra tunneling, not game client communication

### 3. Cloudflare Tunnel (Zero Trust)
- **Pros**: Globally distributed, DDoS protection, zero ingress firewall rules
- **Cons**: Third-party dependency; adds 50–100ms latency; may violate anti-detection requirements (Cloudflare IPs are scannable); overkill for private backend
- **Verdict**: Rejected as primary; consider for staging/demo environment only

## Consequences

### Positive
- ✅ Clients connect via standard HTTPS; no special client setup beyond binary + TLS trust store
- ✅ Real-time bidirectional updates eliminate polling; sub-100ms update latency
- ✅ Horizontal load balancing via standard tools (Nginx, cloud LBs)
- ✅ Automatic reconnection via standard WebSocket libraries (with exponential backoff)
- ✅ Rich debugging (HTTP logs, packet capture, TLS inspection)
- ✅ Supports multi-region failover: DNS round-robin or geo-steering

### Negative
- ⚠️ TLS certificate renewal adds minor ops burden (mitigated by Let's Encrypt automation + certbot)
- ⚠️ Firewall inspection may detect WebSocket upgrades (mitigated via WSS obscuration; low-probability detection given 443 prevalence)
- ⚠️ Client latency sensitive to server→client network path (accept as baseline requirement; <150ms for 95th percentile)

### Migration Path
- Start with single WSS endpoint on backend
- Build auto-reconnect + exponential backoff into dll → web protocol layer
- Deploy dual load balancers (active–passive) on day 1; failover DNS TTL = 60s
- Upgrade to multi-region with Tailscale mesh for ops (secondary, optional)

## Validation Checklist

- [ ] WebSocket TLS endpoint deployed on textquest-web (port 443)
- [ ] Let's Encrypt certificate provisioned (auto-renewal via systemd timer or similar)
- [ ] textquest-dll implements WebSocket client with auto-reconnect (exponential backoff, jitter)
- [ ] Network latency profile measured (99th percentile <200ms target)
- [ ] Firewall blocking tested (corporate proxy, ISP restrictive rules)
- [ ] Load test: 20 concurrent clients, 6s tick update cadence, track message loss
- [ ] Ops runbook: certificate rollover, failover DNS cutover, client state recovery

## Next Steps

1. Implement WSS server in textquest-web (tokio + tungstenite)
2. Implement WSS client in textquest-dll (with auto-reconnect)
3. Define message schema (JSON frames, compressed if >4KB)
4. Deploy to staging; measure latency and reconnect behavior
5. Integration test: Frostreaver Teek multibox (20 clients) vs. WSS endpoint
6. Merge to master; close #2253

## References

- ADR-002: Backend Host Selection (Oracle Free Tier + DigitalOcean fallback)
- Research: adr-backend-transport.md (candidates + detailed evaluation)
- Issue #2249: Transport research spike
