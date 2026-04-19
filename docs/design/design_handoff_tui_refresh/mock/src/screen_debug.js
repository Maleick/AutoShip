// screen_debug.js — Screen 4: Spawns table + hex dump pane.
import { SPAWNS } from "./data.js";
import { panel, pad } from "./widgets.js";
import { W, MAIN_W, SIDEBAR_W } from "./shell.js";

function spawnsTable(selIdx = 0) {
  const cols = [
    { label: "", w: 2 },
    { label: "ID", w: 7, align: "right" },
    { label: "Name", w: 26 },
    { label: "Type", w: 7 },
    { label: "Cls", w: 4 },
    { label: "Lvl", w: 4, align: "right" },
    { label: "HP%", w: 5, align: "right" },
    { label: "Y", w: 8, align: "right" },
    { label: "X", w: 8, align: "right" },
    { label: "Z", w: 6, align: "right" },
    { label: "State", w: 6 },
  ];
  const innerW = MAIN_W - 2;
  const header = cols.map((c) => pad(`<span class="secondary">${c.label}</span>`, c.w, c.align)).join(" ");
  const rows = [header, `<span class="dim">${"─".repeat(innerW)}</span>`];
  SPAWNS.forEach((s, i) => {
    const sel = i === selIdx;
    const cur = sel ? `<span class="magenta bold">▶</span> ` : `  `;
    const typeCol = s.type === "Named" ? "spawn-named" : s.type === "Corpse" ? "muted" : "spawn-npc";
    const name = sel
      ? `<span class="inv-magenta">${pad(s.name, 26)}</span>`
      : `<span class="${typeCol}">${pad(s.name, 26)}</span>`;
    const hpCol = s.hp > 60 ? "hp-high" : s.hp > 30 ? "hp-mid" : s.hp === 0 ? "muted" : "hp-low";
    rows.push(
      `${cur}` +
      pad(`<span class="bright">${s.id}</span>`, 7, "right") + " " +
      name + " " +
      pad(`<span class="${typeCol}">${s.type}</span>`, 7) + " " +
      pad(`<span class="highlight">${s.cls}</span>`, 4) + " " +
      pad(`<span class="bright">${s.lvl}</span>`, 4, "right") + " " +
      pad(`<span class="${hpCol}">${s.hp}</span>`, 5, "right") + " " +
      pad(`<span class="bright">${s.y}</span>`, 8, "right") + " " +
      pad(`<span class="bright">${s.x}</span>`, 8, "right") + " " +
      pad(`<span class="bright">${s.z}</span>`, 6, "right") + " " +
      pad(`<span class="${s.state === "Dead" ? "muted" : "bright"}">${s.state}</span>`, 6)
    );
  });
  rows.push("");
  rows.push(`<span class="muted">filter:</span> <span class="cyan">type=*</span> <span class="muted">·</span> <span class="cyan">hp>0</span> <span class="muted">·</span> showing 9 / 47`);
  return panel(rows, { width: MAIN_W, title: `Spawns · zone Plane of Fear · ${SPAWNS.length} entities`, color: "magenta", foot: "↑↓ select  ·  / filter  ·  t target  ·  i inspect  ·  d dump" });
}

function hexDump() {
  const s = SPAWNS[0];
  const lines = [
    `<span class="secondary">inspect</span> <span class="bright bold">${s.name}</span> <span class="muted">(id ${s.id}, 0x${s.id.toString(16).toUpperCase()})</span>`,
    "",
    `<span class="secondary">struct Spawn</span> <span class="muted">{</span>`,
    `  <span class="cyan">entity_id</span>   = <span class="bright">${s.id}</span>`,
    `  <span class="cyan">name</span>        = <span class="amber">"${s.name}"</span>`,
    `  <span class="cyan">type</span>        = <span class="magenta">SpawnType::Named</span>`,
    `  <span class="cyan">class_id</span>    = <span class="bright">7</span> <span class="muted">/* SHD */</span>`,
    `  <span class="cyan">level</span>       = <span class="bright">${s.lvl}</span>`,
    `  <span class="cyan">hp_pct</span>      = <span class="amber">${s.hp}</span>`,
    `  <span class="cyan">pos</span>         = <span class="bright">(${s.y}.0, ${s.x}.0, ${s.z}.0)</span>`,
    `  <span class="cyan">heading</span>     = <span class="bright">112</span>`,
    `  <span class="cyan">flags</span>       = <span class="magenta">NAMED | AGGRO | SEE_INVIS</span>`,
    `<span class="muted">}</span>`,
    "",
    `<span class="secondary">raw bytes</span> <span class="muted">(first 64)</span>`,
    `<span class="muted">0000</span>  <span class="cyan">f5 47 00 00</span> <span class="bright">43 61 7a 69</span> <span class="bright">63 2d 54 68</span> <span class="bright">75 6c 65 00</span>  <span class="muted">│</span> <span class="amber">.G..Cazic-Thule.</span>`,
    `<span class="muted">0010</span>  <span class="cyan">00 00 00 00</span> <span class="cyan">07 00 00 00</span> <span class="cyan">42 00 00 00</span> <span class="cyan">40 00 00 00</span>  <span class="muted">│</span> <span class="amber">........B...@...</span>`,
    `<span class="muted">0020</span>  <span class="bright">00 00 4b 44</span> <span class="bright">00 00 ae c3</span> <span class="bright">00 00 40 40</span> <span class="bright">70 00 00 00</span>  <span class="muted">│</span> <span class="amber">..KD......@@p...</span>`,
    `<span class="muted">0030</span>  <span class="magenta">0e 00 00 00</span> <span class="magenta">00 00 00 00</span> <span class="magenta">00 00 00 00</span> <span class="magenta">00 00 00 00</span>  <span class="muted">│</span> <span class="amber">................</span>`,
  ];
  return panel(lines, { width: SIDEBAR_W, title: "Inspect · hex dump", color: "cyan", titleColor: "bright" });
}

function composeTwo(left, right) {
  const L = left.split("\n"), R = right.split("\n");
  const h = Math.max(L.length, R.length);
  const out = [];
  for (let i = 0; i < h; i++) out.push((L[i] ?? " ".repeat(MAIN_W)) + " " + (R[i] ?? " ".repeat(SIDEBAR_W)));
  return out.join("\n");
}

export function renderDebug() {
  return composeTwo(spawnsTable(0), hexDump());
}
