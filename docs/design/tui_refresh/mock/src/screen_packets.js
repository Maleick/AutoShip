// screen_packets.js — Screen 5: Packet stream + filter bar + detail.
import { PACKETS } from "./data.js";
import { panel, pad } from "./widgets.js";
import { W, MAIN_W, SIDEBAR_W } from "./shell.js";

function streamTable(selIdx = 5) {
  const cols = [
    { label: "", w: 2 },
    { label: "Time", w: 13 },
    { label: "Dir", w: 4 },
    { label: "Opcode", w: 22 },
    { label: "Size", w: 5, align: "right" },
    { label: "Payload (first 16 bytes)", w: 50 },
  ];
  const innerW = MAIN_W - 2;
  const header = cols.map((c) => pad(`<span class="secondary">${c.label}</span>`, c.w, c.align)).join(" ");
  const rows = [header, `<span class="dim">${"─".repeat(innerW)}</span>`];
  PACKETS.forEach((p, i) => {
    const sel = i === selIdx;
    const cur = sel ? `<span class="magenta bold">▶</span> ` : `  `;
    const dirCol = p.dir === "S→C" ? "cyan" : "green";
    const opCol = p.op.includes("HP") || p.op.includes("Mana") ? "amber" : p.op.includes("Cast") ? "magenta" : p.op.includes("Damage") ? "red" : "bright";
    const name = sel
      ? `<span class="inv-magenta">${pad(p.op, 22)}</span>`
      : `<span class="${opCol}">${pad(p.op, 22)}</span>`;
    rows.push(
      `${cur}` +
      pad(`<span class="secondary">${p.t}</span>`, 13) + " " +
      pad(`<span class="${dirCol}">${p.dir}</span>`, 4) + " " +
      name + " " +
      pad(`<span class="bright">${p.size}</span>`, 5, "right") + " " +
      pad(`<span class="muted">${p.hex}</span>`, 50)
    );
  });
  rows.push("");
  rows.push(`<span class="secondary">filter:</span> <span class="cyan">op=*</span> <span class="muted">·</span> <span class="cyan">dir=any</span> <span class="muted">·</span> <span class="cyan">client=*</span> <span class="muted">·</span> <span class="green">▶ live</span>  <span class="muted">(${PACKETS.length} rows · 1,248 since 11:42)</span>`);
  return panel(rows, { width: MAIN_W, title: "Packet Stream · 1,248 captured · 12,482/s peak", color: "magenta", foot: "↑↓ select  ·  / filter  ·  p pause  ·  space mark  ·  e export" });
}

function packetDetail() {
  const p = PACKETS[5];
  const lines = [
    `<span class="secondary">captured</span> <span class="bright">${p.t}</span>   <span class="secondary">dir</span> <span class="cyan">${p.dir}</span>`,
    `<span class="secondary">opcode  </span> <span class="magenta bold">${p.op}</span> <span class="muted">(0x${(0x4100).toString(16).toUpperCase()})</span>`,
    `<span class="secondary">size    </span> <span class="bright">${p.size} bytes</span>`,
    `<span class="secondary">client  </span> <span class="highlight">Sylunariel</span> <span class="muted">(pid 4826)</span>`,
    "",
    `<span class="secondary">decoded</span>`,
    `  <span class="cyan">caster_id</span>    = <span class="bright">4826</span>  <span class="muted">// Sylunariel</span>`,
    `  <span class="cyan">target_id</span>    = <span class="bright">4829</span>  <span class="muted">// Thurgrek</span>`,
    `  <span class="cyan">spell_id</span>     = <span class="bright">1000</span>  <span class="muted">// Complete Healing</span>`,
    `  <span class="cyan">cast_time_ms</span> = <span class="bright">10000</span>`,
    `  <span class="cyan">gem_slot</span>     = <span class="bright">7</span>`,
    "",
    `<span class="secondary">hex</span>`,
    `<span class="muted">0000</span>  ${p.hex}`,
  ];
  return panel(lines, { width: SIDEBAR_W, title: "Packet · detail", color: "cyan", titleColor: "bright" });
}

function composeTwo(left, right) {
  const L = left.split("\n"), R = right.split("\n");
  const h = Math.max(L.length, R.length);
  const out = [];
  for (let i = 0; i < h; i++) out.push((L[i] ?? " ".repeat(MAIN_W)) + " " + (R[i] ?? " ".repeat(SIDEBAR_W)));
  return out.join("\n");
}

export function renderPackets() { return composeTwo(streamTable(), packetDetail()); }
