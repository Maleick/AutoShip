// screen_navigation.js — Screen 3: per-client route cards + blocker summary.

import { CLIENTS, NAV } from "./data.js";
import { panel, pad, bar } from "./widgets.js";
import { W } from "./shell.js";

const CARD_W = Math.floor((W - 2) / 2); // two cards per row, -2 for gap

function navCard(c) {
  const nav = NAV[c.pid];
  const statusColor =
    nav.status === "Stuck" ? "red" :
    nav.status === "Holding" ? "amber" :
    nav.status === "Moving" ? "cyan" : "secondary";
  const eta = nav.status === "Moving" ? "12.4s" : nav.status === "Holding" ? "at anchor" : "—";

  const lines = [
    `<span class="secondary">Class    </span> <span class="highlight">${c.clsFull}</span>  <span class="muted">L${c.lvl}</span>   <span class="secondary">Group </span><span class="cyan">${c.group}</span>`,
    `<span class="secondary">Zone     </span> <span class="bright">${c.zone}</span>`,
    `<span class="secondary">Position </span> <span class="bright">${c.pos.y.toFixed(1)}, ${c.pos.x.toFixed(1)}, ${c.pos.z.toFixed(1)}</span>  <span class="muted">h${c.pos.h}</span>`,
    "",
    `<span class="secondary">Status   </span> <span class="${statusColor} bold">${nav.status}</span>   <span class="secondary">ETA </span><span class="bright">${eta}</span>`,
    `<span class="secondary">Destination</span> <span class="cyan">${nav.dest}</span>`,
    `<span class="secondary">Route    </span> <span class="secondary">${nav.route || "—"}</span>`,
  ];

  if (nav.stuck) {
    lines.push("");
    lines.push(`<span class="red bold">⚠ Blocker:</span> <span class="bright">${nav.blocker}</span>`);
    lines.push(`<span class="muted">  retries 3/5 · fallback route queued · :nav unstick</span>`);
  } else {
    // Mini route progress
    lines.push("");
    lines.push(`<span class="secondary">Progress</span> ${bar(nav.status === "Holding" ? 1.0 : 0.42, 1, 28, { color: "cyan" })} <span class="bright">${nav.status === "Holding" ? "100%" : "42%"}</span>`);
  }

  const titleColor = nav.status === "Stuck" ? "red" : "magenta";
  return panel(lines, { width: CARD_W, title: `${c.name} · ${c.cls}`, color: titleColor });
}

function composeGrid(cards) {
  // Pair cards two per row, left + " " + right
  const rows = [];
  for (let i = 0; i < cards.length; i += 2) {
    const L = cards[i].split("\n");
    const R = (cards[i + 1] || "").split("\n");
    const h = Math.max(L.length, R.length);
    const out = [];
    for (let j = 0; j < h; j++) {
      const l = L[j] ?? " ".repeat(CARD_W);
      const r = R[j] ?? " ".repeat(CARD_W);
      out.push(l + " " + r);
    }
    rows.push(out.join("\n"));
  }
  return rows.join("\n");
}

function blockerSummary() {
  const lines = [
    `<span class="red bold">1 blocker</span> · <span class="amber">1 stuck</span> · <span class="green">5 routing nominal</span>`,
    "",
    `<span class="red">⚠</span> <span class="bright">Kardek</span> <span class="muted">slot S06 · Plane of Fear</span>`,
    `    <span class="secondary">blocker   </span> <span class="bright">obstacle: corpse pile @ (821,-1420)</span>`,
    `    <span class="secondary">last move </span> <span class="bright">3.2s ago</span>   <span class="secondary">retries </span><span class="amber">3/5</span>`,
    `    <span class="secondary">fallback  </span> <span class="cyan">mesh/fallback → fear.zone_in</span>`,
    `    <span class="muted">resolution options: :nav unstick · :nav reroute · :nav force_tp</span>`,
  ];
  return panel(lines, { width: W, title: "Nav Blockers", color: "red" });
}

export function renderNavigation() {
  const cards = CLIENTS.map(navCard);
  return blockerSummary() + "\n" + composeGrid(cards);
}
