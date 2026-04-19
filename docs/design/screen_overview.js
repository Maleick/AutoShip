// screen_overview.js — Screen 1: Characters / Ops Roster.
//
// Layout:
//   ┌ Group Focus strip ────────────────────────────────────────────┐
//   ┌ Ops Roster ────────────────────────┬ Character · Venkhadrei ──┐
//   │  ▶ Name       Grp Cls  Zone   HP   │  [ Condition box ]       │
//   │    ...rows...                      │  [ Target / Cast box ]   │
//   │                                    │  [ Groups box ]          │
//   │                                    │  [ Session box ]         │
//   │                                    │  [ Combat box ]          │
//   └────────────────────────────────────┴──────────────────────────┘

import { CLIENTS, GROUPS, SESSION } from "./data.js";
import { BOX, panel, pad, vlen, hpCell, bar } from "./widgets.js";
import { W, MAIN_W, SIDEBAR_W } from "./shell.js";

const ACTIVITY_LEGEND = [
  ["➜", "Nav", "cyan"],
  ["✓", "Arr", "green"],
  ["!", "Stk", "amber"],
  ["☠", "Ded", "red"],
  ["⇣", "FD ", "blue"],
  ["☾", "Sit", "secondary"],
  ["⌕", "Lot", "magenta"],
  ["✦", "Cst", "cyan"],
  ["⚔", "Fgt", "amber"],
  ["●", "Rdy", "green"],
];

// ─── Roster table ────────────────────────────────────────────────────
function rosterTable(selectedIdx) {
  const cols = [
    { label: "",      w: 2  },  // cursor
    { label: "Name",  w: 14 },
    { label: "Grp",   w: 3  },
    { label: "Cls",   w: 4  },
    { label: "Lvl",   w: 3, align: "right" },
    { label: "Zone",  w: 22 },
    { label: "HP",    w: 16 },
    { label: "Mana",  w: 12 },
    { label: "Cond",  w: 9  },
    { label: "State", w: 8  },
    { label: "Activity", w: 10 },
  ];
  const usedW = cols.reduce((a, c) => a + c.w, 0) + (cols.length - 1); // 1-space gaps
  const innerW = MAIN_W - 2;

  const header = cols
    .map((c) => pad(`<span class="secondary">${c.label}</span>`, c.w, c.align || "left"))
    .join(" ");

  const rows = [header, `<span class="dim">${"─".repeat(innerW)}</span>`];

  CLIENTS.forEach((c, i) => {
    const sel = i === selectedIdx;
    const cur = sel ? `<span class="magenta bold">▶</span> ` : `  `;
    const name = sel
      ? `<span class="inv-magenta">${pad(c.name, 14)}</span>`
      : `<span class="bright">${pad(c.name, 14)}</span>`;
    const grp = `<span class="cyan">${pad(c.group, 3)}</span>`;
    const cls = `<span class="highlight">${pad(c.cls, 4)}</span>`;
    const lvl = pad(`<span class="bright">${c.lvl}</span>`, 3, "right");
    const zone = pad(`<span class="secondary">${c.zone}</span>`, 22);

    // HP: "96% ███▌···"
    const hpStr = hpCell(c.hp, 10);
    const hpW = 16;
    const hpCell2 = pad(hpStr, hpW);

    // Mana (only for caster classes — WAR/MNK show --)
    const hasMana = !["WAR", "MNK", "ROG", "BER"].includes(c.cls);
    const manaStr = hasMana
      ? `<span class="mana">${String(c.mana).padStart(3)}%</span> ${bar(c.mana, 100, 6, { mana: true })}`
      : `<span class="muted">  -- </span>`;
    const manaW = 12;
    const manaC = pad(manaStr, manaW);

    const condColor =
      c.cond === "Critical" ? "red" :
      c.cond === "Hurt" ? "amber" :
      c.cond === "Stable" ? "green" : "secondary";
    const cond = pad(`<span class="${condColor}">${c.cond}</span>`, 9);

    const stateColor = c.state === "FD" ? "blue" : c.state === "Sit" ? "secondary" : "bright";
    const state = pad(`<span class="${stateColor}">${c.state}</span>`, 8);

    const actColor =
      c.activity.startsWith("⚔") ? "amber" :
      c.activity.startsWith("✦") ? "cyan" :
      c.activity.startsWith("⇣") ? "blue" :
      c.activity.startsWith("●") ? "green" :
      c.activity.startsWith("☾") ? "secondary" : "bright";
    const act = pad(`<span class="${actColor}">${c.activity}</span>`, 10);

    const line = `${cur}${name} ${grp} ${cls} ${lvl} ${zone} ${hpCell2} ${manaC} ${cond} ${state} ${act}`;
    rows.push(line);
  });

  // Legend footer inside the panel
  rows.push("");
  const legend = ACTIVITY_LEGEND
    .map(([g, l, col]) => `<span class="${col}">${g}</span> <span class="secondary">${l}</span>`)
    .join("  ");
  rows.push(`<span class="muted">Activity glyphs:</span>  ${legend}`);

  return panel(rows, {
    width: MAIN_W,
    title: `Ops Roster · ${CLIENTS.length} clients · sorted by Group`,
    color: "magenta",
    foot: "↑↓ select  ·  Enter focus  ·  g cycle group  ·  [ ] prev/next client",
  });
}

// ─── Sidebar sections ────────────────────────────────────────────────
function condBox(c) {
  const lines = [];
  const hasMana = !["WAR", "MNK"].includes(c.cls);
  lines.push(`<span class="secondary">HP  </span>${hpCell(c.hp, 18)}`);
  if (hasMana) {
    lines.push(`<span class="secondary">Mana</span> <span class="mana">${String(c.mana).padStart(3)}%</span> ${bar(c.mana, 100, 18, { mana: true })}`);
  } else {
    lines.push(`<span class="secondary">Mana</span> <span class="muted">— (melee) —</span>`);
  }
  lines.push(`<span class="secondary">End </span> <span class="green">${String(c.endu).padStart(3)}%</span> ${bar(c.endu, 100, 18, { color: "green" })}`);
  lines.push("");
  lines.push(`<span class="secondary">State</span>     <span class="bright">${c.state}</span>`);
  lines.push(`<span class="secondary">Activity</span>  <span class="bright">${c.activity}</span>`);
  lines.push(`<span class="secondary">Condition</span> <span class="${c.cond === "Critical" ? "red" : c.cond === "Hurt" ? "amber" : "green"} bold">${c.cond}</span>`);
  lines.push(`<span class="secondary">Position</span>  <span class="bright">${c.pos.y.toFixed(1)}, ${c.pos.x.toFixed(1)}, ${c.pos.z.toFixed(1)}</span> <span class="muted">h${c.pos.h}</span>`);
  return panel(lines, {
    width: SIDEBAR_W,
    title: `Character · ${c.name}`,
    color: "cyan",
    titleColor: "bright",
  });
}

function castBox(c) {
  const lines = [];
  if (c.target) {
    const tCol = c.target.type === "Named" ? "magenta" : c.target.type === "PC" ? "green" : "bright";
    lines.push(`<span class="secondary">Target</span>    <span class="${tCol} bold">${c.target.name}</span>`);
    lines.push(`<span class="secondary">Target HP</span> ${hpCell(c.target.hp, 18)}`);
  } else {
    lines.push(`<span class="secondary">Target</span>    <span class="muted">none</span>`);
  }
  lines.push("");
  if (c.cast) {
    const pct = Math.min(1, c.cast.elapsed / (c.cast.elapsed + c.cast.remaining));
    const exact = c.cast.exact ? "" : ` <span class="muted">±</span>`;
    lines.push(`<span class="secondary">Casting</span>   <span class="cyan bold">${c.cast.label}</span> <span class="muted">${c.cast.gem}</span>`);
    lines.push(`<span class="secondary">Progress</span>  ${bar(pct, 1, 22, { color: "cyan" })} <span class="bright">${Math.round(pct * 100)}%</span>`);
    lines.push(`<span class="secondary">Remaining</span> <span class="bright">${c.cast.remaining.toFixed(1)}s</span>${exact}`);
  } else {
    lines.push(`<span class="secondary">Casting</span>   <span class="muted">idle</span>`);
  }
  return panel(lines, {
    width: SIDEBAR_W,
    title: `Target · Cast`,
    color: "magenta",
  });
}

function groupsBox() {
  const lines = [];
  GROUPS.forEach((g) => {
    const active = g.id === 2;
    const marker = active ? `<span class="inv-magenta"> ${g.id === 1 ? "G1" : "G2"} </span>` : `<span class="muted">[${g.id === 1 ? "G1" : "G2"}]</span>`;
    lines.push(`${marker} <span class="bright bold">${g.label.slice(3)}</span>`);
    lines.push(`   <span class="secondary">Zone </span><span class="bright">${g.zone}</span>`);
    lines.push(`   <span class="secondary">Conn </span><span class="${g.connected === g.members ? "green" : "amber"}">${g.connected}/${g.members}</span>   <span class="secondary">Lead </span><span class="highlight">${g.leader}</span>`);
  });
  return panel(lines, { width: SIDEBAR_W, title: "Groups", color: "magenta" });
}

function sessionBox() {
  const uptime = `${String(SESSION.elapsed.h).padStart(2, "0")}:${String(SESSION.elapsed.m).padStart(2, "0")}:${String(SESSION.elapsed.s).padStart(2, "0")}`;
  const lines = [
    `<span class="secondary">Server  </span> <span class="server bold">${SESSION.server}</span>`,
    `<span class="secondary">Mode    </span> <span class="${SESSION.mode === "Hunt" ? "magenta" : "cyan"} bold">${SESSION.mode}</span>`,
    `<span class="secondary">Uptime  </span> <span class="bright">${uptime}</span>`,
    "",
    `<span class="secondary">Kills   </span> <span class="bright bold">${SESSION.kills}</span>  <span class="secondary">Deaths </span><span class="red">${SESSION.deaths}</span>`,
    `<span class="secondary">XP      </span> <span class="bright">${(SESSION.xp / 1_000_000).toFixed(2)}M</span>   <span class="green">+${(SESSION.xpPerHour / 1000).toFixed(0)}k/h</span>`,
    `<span class="secondary">XP 15m  </span> <span class="cyan">+${(SESSION.xp15m / 1000).toFixed(0)}k</span>`,
    `<span class="secondary">Plat    </span> <span class="bright">${SESSION.plat.toFixed(1)}</span>    <span class="green">+${SESSION.platPerHour.toFixed(1)}/h</span>`,
  ];
  return panel(lines, { width: SIDEBAR_W, title: "Session", color: "magenta" });
}

function combatBox() {
  const ch = SESSION.chChain;
  const lines = [
    `<span class="secondary">Main Assist</span>  <span class="highlight bold">${SESSION.mainAssist}</span>`,
    `<span class="secondary">Main Tank  </span>  <span class="highlight bold">${SESSION.mainTank}</span>`,
    "",
    `<span class="secondary">CH Chain</span>     <span class="${ch.members >= 3 ? "green" : "amber"} bold">${ch.members} clerics</span> <span class="muted">@</span> <span class="bright">${ch.intervalSecs.toFixed(1)}s</span>`,
    `<span class="secondary">Adaptive</span>     <span class="${ch.adaptive ? "green" : "muted"}">${ch.adaptive ? "ON" : "off"}</span>`,
    `<span class="secondary">Chain Target</span> <span class="highlight">${SESSION.mainTank}</span>`,
    "",
    `<span class="secondary">Last CH</span>      <span class="bright">CH#142</span> <span class="muted">fired 12:04:16.5</span>`,
    `<span class="secondary">Next CH</span>      <span class="cyan">CH#143</span> <span class="muted">in 2.4s</span>`,
  ];
  return panel(lines, { width: SIDEBAR_W, title: "Combat", color: "magenta" });
}

// ─── Side-by-side composition ────────────────────────────────────────
function composeTwoColumn(left, right) {
  const L = left.split("\n");
  const R = right.split("\n");
  const rows = Math.max(L.length, R.length);
  const blankL = " ".repeat(MAIN_W);
  const blankR = " ".repeat(SIDEBAR_W);
  const out = [];
  for (let i = 0; i < rows; i++) {
    const l = L[i] ?? blankL;
    const r = R[i] ?? blankR;
    // Pad left to MAIN_W visible chars (it already is, but safety)
    out.push(l + " " + r);
  }
  return out.join("\n");
}

export function renderOverview(selectedIdx = 2) {
  const c = CLIENTS[selectedIdx];
  const sidebar = [condBox(c), castBox(c), groupsBox(), sessionBox(), combatBox()].join("\n");
  return composeTwoColumn(rosterTable(selectedIdx), sidebar);
}
