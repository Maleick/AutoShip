// shell.js — the header bar, tab strip, and status bar that wrap every screen.

import { CLIENTS, SESSION, ALERTS } from "./data.js";
import { BOX, pad, vlen } from "./widgets.js";

export const W = 146; // overall terminal width in chars
export const SIDEBAR_W = 44; // right-hand column for Overview / Tactical
export const MAIN_W = W - SIDEBAR_W - 1; // -1 for the gap

// ─── Header: `╭─ TextQuest ─────...────╮` with inline tab strip ────────
export function renderHeader(activeTab) {
  const tabs = [
    { n: 1, short: "Char", long: "Characters" },
    { n: 2, short: "Map", long: "Tactical" },
    { n: 3, short: "Nav", long: "Navigation" },
    { n: 4, short: "Dbg", long: "Debug" },
    { n: 5, short: "Pkt", long: "Packets" },
    { n: 6, short: "Eco", long: "Economy" },
    { n: 7, short: "Orc", long: "Orchestra" },
  ];

  const b = BOX.rounded;
  const titleText = ` TextQuest `;

  // Build the tab strip: `[ 1 Char ][ 2 Map ]...`
  const tabStr = tabs
    .map((t) => {
      const active = t.n === activeTab;
      const inner = `${t.n} ${t.short}`;
      if (active) {
        return `<span class="magenta">[</span><span class="inv-magenta"> ${inner} </span><span class="magenta">]</span>`;
      }
      return `<span class="muted">[</span><span class="secondary"> ${inner} </span><span class="muted">]</span>`;
    })
    .join("");

  // Top border with title
  const topLead = `<span class="magenta">${b.tl}${b.h}${b.h}</span><span class="bright bold">${titleText}</span>`;
  const topLeadW = 2 + titleText.length;
  const topTrailW = W - topLeadW - 2; // minus the corners counted above
  const topLine = topLead + `<span class="magenta">${b.h.repeat(topTrailW)}${b.tr}</span>`;

  // Middle line: left meta + right tabs
  const left =
    ` <span class="cyan bold">${CLIENTS.length}x</span><span class="secondary"> EQ</span>` +
    ` <span class="muted">│</span> ` +
    `<span class="bright">1/${CLIENTS.length}</span> <span class="highlight">${CLIENTS[0].name}</span>` +
    ` <span class="muted">│</span> ` +
    `<span class="cyan">${SESSION.focusGroup}</span> <span class="secondary">${GROUP_LABEL(SESSION.focusGroup)}</span>` +
    ` <span class="muted">│</span> ` +
    `<span class="server bold">${SESSION.server}</span>` +
    ` <span class="muted">│</span> ` +
    `<span class="bright">${CLIENTS[0].zone}</span> `;

  const right = ` ${tabStr} `;
  const wLeft = vlen(left);
  const wRight = vlen(right);
  const mid = wLeft + wRight + 2 > W ? "" : " ".repeat(W - 2 - wLeft - wRight);
  const midLine =
    `<span class="magenta">${b.v}</span>` + left + mid + right + `<span class="magenta">${b.v}</span>`;

  // Bottom border of header panel — we'll continue into the screen body below
  // so use ├───┤ so the screen panels beneath can visually attach. But simpler:
  // draw a closed header and leave a blank line before the body.
  const botLine = `<span class="magenta">${b.bl}${b.h.repeat(W - 2)}${b.br}</span>`;

  return [topLine, midLine, botLine].join("\n");
}

function GROUP_LABEL(id) {
  return id === "G1" ? "Neriak Camp" : id === "G2" ? "Fear Core" : id;
}

// ─── Status bar at the bottom of every screen ────────────────────────
export function renderStatusBar({ hint, screen, activeTab }) {
  const unread = ALERTS.filter((a) => !a.ack).length;
  const chBadge =
    `<span class="cyan">CH </span>` +
    `<span class="bright">${SESSION.chChain.members}x@${SESSION.chChain.intervalSecs.toFixed(1)}s</span> ` +
    `<span class="${SESSION.chChain.adaptive ? "green" : "muted"}">${SESSION.chChain.adaptive ? "A" : "·"}</span>`;

  const modeBadge = SESSION.mode === "Hunt"
    ? `<span class="inv-magenta"> HUNT </span>`
    : `<span class="inv-cyan"> CAMP </span>`;

  const focusPill = `<span class="magenta">[</span><span class="inv-magenta"> ${SESSION.focusGroup} </span><span class="magenta">]</span>`;

  const left =
    ` <span class="bright bold">${screen}</span> ` +
    `<span class="muted">▸</span> ` +
    `<span class="secondary">${hint}</span>`;

  const right =
    ` ${modeBadge} ` +
    `<span class="muted">│</span> ` +
    `<span class="secondary">MA </span><span class="bright">${SESSION.mainAssist}</span> ` +
    `<span class="muted">│</span> ` +
    `<span class="secondary">MT </span><span class="bright">${SESSION.mainTank}</span> ` +
    `<span class="muted">│</span> ` +
    `${chBadge} ` +
    `<span class="muted">│</span> ` +
    `${focusPill} ` +
    `<span class="muted">│</span> ` +
    `<span class="secondary">Alerts </span><span class="${unread ? "amber" : "muted"}">${unread}</span> ` +
    `<span class="muted">│</span> ` +
    `<span class="muted">neriak</span> `;

  const wLeft = vlen(left);
  const wRight = vlen(right);
  const fill = Math.max(1, W - wLeft - wRight);
  return `<span class="dim">${"─".repeat(W)}</span>\n` + left + " ".repeat(fill) + right;
}

// ─── Session strip (Group Focus bar) — pinned above Overview ─────────
export function renderFocusStrip() {
  const b = BOX.rounded;
  const title = ` Group Focus · ${SESSION.focusGroup} ${GROUP_LABEL(SESSION.focusGroup)} `;
  const topLead = `<span class="cyan">${b.tl}${b.h}${b.h}</span>` + `<span class="bright bold">${title}</span>`;
  const topLeadW = 2 + title.length;
  const top = topLead + `<span class="cyan">${b.h.repeat(W - topLeadW - 1)}${b.tr}</span>`;

  const uptime = `${String(SESSION.elapsed.h).padStart(2, "0")}:${String(SESSION.elapsed.m).padStart(2, "0")}:${String(SESSION.elapsed.s).padStart(2, "0")}`;
  const inner =
    ` <span class="secondary">Uptime </span><span class="bright">${uptime}</span>` +
    `   <span class="muted">│</span>   ` +
    `<span class="secondary">Kills </span><span class="bright bold">${SESSION.kills}</span>` +
    ` <span class="muted">/</span> <span class="secondary">Deaths </span><span class="red">${SESSION.deaths}</span>` +
    `   <span class="muted">│</span>   ` +
    `<span class="secondary">XP </span><span class="bright">${(SESSION.xp / 1_000_000).toFixed(2)}M</span>` +
    ` <span class="secondary">·</span> <span class="green">+${(SESSION.xpPerHour / 1000).toFixed(0)}k/h</span>` +
    ` <span class="secondary">·</span> <span class="cyan">+${(SESSION.xp15m / 1000).toFixed(0)}k/15m</span>` +
    `   <span class="muted">│</span>   ` +
    `<span class="secondary">Plat </span><span class="bright">${SESSION.plat.toFixed(1)}</span>` +
    ` <span class="secondary">·</span> <span class="green">+${SESSION.platPerHour.toFixed(1)}/h</span>` +
    `   <span class="muted">│</span>   ` +
    `<span class="secondary">Top Loot </span>` +
    SESSION.topLoot.map(([n, q]) => `<span class="bright">${n}</span><span class="muted">×${q}</span>`).join(" <span class=muted>·</span> ") +
    ` `;

  const bot = `<span class="cyan">${b.bl}${b.h.repeat(W - 2)}${b.br}</span>`;

  const vlenInner = vlen(inner);
  const fill = Math.max(1, W - 2 - vlenInner);
  return [
    top,
    `<span class="cyan">${b.v}</span>` + inner + " ".repeat(fill) + `<span class="cyan">${b.v}</span>`,
    bot,
  ].join("\n");
}
