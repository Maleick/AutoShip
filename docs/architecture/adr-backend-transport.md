# ADR: Backend Transport Mechanism (textquest-dll ↔ textquest-web)

**Status:** Draft  
**Issue:** #2249  
**Decision Date:** 2026-04-25  

## Problem Statement

The textquest-dll (Windows) must establish a persistent, low-latency outbound channel to reach a remotely-hosted textquest-web backend. The connection must:

1. **Not require inbound firewall holes on Frostreaver** — must initiate outbound only
2. **Survive game-tick timing constraints** — ~6 second EQ server ticks mean reconnection must be sub-tick
3. **Work on Frostreaver's home ISP connection** — flaky, possibly behind double NAT
4. **Support persistent state** — control commands, DLL state push, minimal overhead

## Candidates Evaluated

### 1. Tailscale Mesh VPN

**How it works:**  
Authenticated overlay network built on WireGuard. Frostreaver + backend both join a Tailnet. Automatic peer discovery via authentication server.

| Attribute | Score |
|-----------|-------|
| **Setup Complexity** | Medium. Requires Tailscale daemon + account. ~5 min per machine. |
| **NAT Traversal** | Excellent. Automatic relay fallback for symmetric NAT. |
| **Latency Overhead** | Low-Medium (~5-10ms typical, relay adds 20-40ms). |
| **Reconnection Behavior** | Auto-reconnect via relay; fast failover (< 1s). |
| **Encryption** | WireGuard-grade (Curve25519 + ChaCha20). Mutual TLS by default. |
| **Auth Model** | Identity-based (user/device); no raw IP exposure. |
| **Lock-in** | Medium. Vendor lock-in to Tailscale's relay infrastructure. |
| **Observability** | Good. Admin console + basic metrics. |

**Pros:**
- Zero-config NAT traversal (best-in-class)
- Stable, production-proven
- No need to expose Frostreaver to internet

**Cons:**
- Requires paid tier for >3 devices on free plan
- Relay costs bandwidth if direct connection fails
- Managed service dependency

**Suitability for Frostreaver↔backend:** ⭐⭐⭐⭐ (Recommended for production)

---

### 2. WireGuard Direct

**How it works:**  
Lightweight VPN. Frostreaver and backend exchange public keys + endpoints. One initiates; both peer directly via UDP port 51820.

| Attribute | Score |
|-----------|-------|
| **Setup Complexity** | High. Manual key exchange, static IP/NAT rule setup required. |
| **NAT Traversal** | Poor. Requires initiator to have public IP or static port-forward rule. |
| **Latency Overhead** | Very Low (~2-5ms). Minimal encryption overhead. |
| **Reconnection Behavior** | Manual intervention if endpoint changes (e.g., ISP reassigns IP). |
| **Encryption** | Excellent (WireGuard kernel-space crypto). |
| **Auth Model** | Key-based; no identity layer (raw peer IPs). |
| **Lock-in** | None. Open protocol, portable across OS. |
| **Observability** | Minimal. Basic stats via `wg` CLI. |

**Pros:**
- Minimal latency + CPU overhead
- No lock-in; fully portable
- Battle-tested protocol

**Cons:**
- Breaks if Frostreaver ISP IP changes (frequent on home connections)
- Requires static NAT rule configuration
- No automatic failover; manual troubleshooting

**Suitability for Frostreaver↔backend:** ⭐⭐ (High maintenance; breaks on ISP restart)

---

### 3. WebSocket over TLS (WSS)

**How it works:**  
Standard HTTPS upgrade to WebSocket. DLL connects to backend via persistent TCP on port 443. TLS for encryption; HTTP Basic/JWT for auth.

| Attribute | Score |
|-----------|-------|
| **Setup Complexity** | Low. Standard HTTPS cert + WebSocket server. Works out-of-box. |
| **NAT Traversal** | Excellent. Port 443 traverses nearly all NAT/proxy. |
| **Latency Overhead** | Low (~10-20ms including TLS handshake). |
| **Reconnection Behavior** | Auto-reconnect via TCP backoff. Browser-like retry logic. |
| **Encryption** | TLS 1.3 (industry standard). Cert validation required. |
| **Auth Model** | Token-based (JWT/API key). Simple, familiar. |
| **Lock-in** | None. Standard protocol, portable across platforms. |
| **Observability** | Excellent. Works with standard HTTP middleware + logging. |

**Pros:**
- Simplest to implement (standard web stack)
- 443 is nearly universally open
- Full HTTP ecosystem (load balancing, observability, debugging)
- No special VPN daemon

**Cons:**
- Observable to network (looks like HTTPS traffic, but protocol is unencrypted above TLS)
- Relies on DNS + certificate renewal
- TCP has higher reconnection overhead than UDP VPNs

**Suitability for Frostreaver↔backend:** ⭐⭐⭐⭐⭐ (Best for simplicity + reliability)

---

### 4. Cloudflare Tunnel (Zero Trust)

**How it works:**  
Frostreaver runs cloudflared daemon (outbound-only connection to Cloudflare). Backend also runs cloudflared or accesses via Cloudflare-issued DNS. All traffic routes through Cloudflare's edge.

| Attribute | Score |
|-----------|-------|
| **Setup Complexity** | Medium. Daemon install + DNS + access policies. ~15 min. |
| **NAT Traversal** | Excellent. Pure outbound (no inbound needed). |
| **Latency Overhead** | Medium (~20-50ms depending on Cloudflare POP). |
| **Reconnection Behavior** | Auto-reconnect via Cloudflare relay; fast (< 1s). |
| **Encryption** | TLS 1.3 + mTLS for device identity. |
| **Auth Model** | Device identity + Cloudflare Access policies. Fine-grained. |
| **Lock-in** | High. Requires Cloudflare account + DNS delegation. |
| **Observability** | Excellent. Cloudflare analytics + access logs. |

**Pros:**
- Zero inbound firewall holes needed
- Built-in DDoS + bot protection
- Fine-grained access policies

**Cons:**
- Strong lock-in to Cloudflare platform
- Outbound bandwidth metered
- Adds ~20-50ms latency

**Suitability for Frostreaver↔backend:** ⭐⭐⭐ (Good for zero-inbound requirement, but over-engineered for p2p use case)

---

## Recommendation

**Choose: WebSocket over TLS (WSS)** ✅

### Reasoning

1. **Simplicity wins:** The DLL is already writing code; adding persistent TCP + JSON over WebSocket requires no new infrastructure.
2. **Reliability on home ISP:** Port 443 survives ISP IP changes, flaky connections, and double NAT without reconfiguration.
3. **Game-tick compatibility:** TCP + exponential backoff means reconnection is reliable within 1-6 second tolerance.
4. **No vendor lock-in:** Standard protocol; trivial to migrate away if needed.
5. **Full observability:** Works with all standard HTTP debugging tools (curl, Wireshark, HAR logs, etc.).

### Secondary: Tailscale for Ops Convenience

If Frostreaver↔backend connection is **not** a game-critical path (e.g., state sync that can tolerate minutes of lag), **also deploy Tailscale** for ops convenience:
- SSH access to Frostreaver for debugging
- Optional fallback route if WebSocket breaks
- Better observability of peer network

### Reject WireGuard

Manual configuration breaks every time Frostreaver's ISP IP rotates (unpredictable on home networks). Not suitable without a dynamic DNS + monitoring layer.

### Reject Cloudflare Tunnel as Primary

Over-engineered for a single DLL↔backend connection. Introduces latency and platform lock-in. Reserve for multi-tenant or public-facing APIs later.

---

## Implementation Plan

1. **Phase 1 (MVP):** WSS + token auth
   - DLL: Persistent TCP client, auto-reconnect with backoff
   - Backend: Standard WebSocket server, JWT validation
   - Testing: Simulate ISP restart, check reconnection time

2. **Phase 2 (Optional):** Tailscale for ops
   - Deploy Tailscale on both machines
   - Use for SSH/debugging; keep WSS as primary control plane

3. **Phase 3 (Future):** Consider Cloudflare if scaling to multi-region

---

## Acceptance Criteria Checklist

- [x] All 4 candidates evaluated (Tailscale, WireGuard, WSS, Cloudflare)
- [x] Comparison table with latency, setup, auth, reliability, lock-in
- [x] Recommendation stated with reasoning
- [x] Secondary/fallback strategies noted
