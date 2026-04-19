// screen_tactical.js — Screen 2: Tactical map + spawn list.
//
// Left: ASCII zone map of Plane of Fear with PC markers, named mobs, corpses.
// Right: spawn list grouped by type, plus target + CH status panels.

import { CLIENTS, SPAWNS, SESSION } from "./data.js";
import { panel, pad, vlen, bar, hpCell, BOX } from "./widgets.js";
import { W, MAIN_W, SIDEBAR_W } from "./shell.js";

// ─── ASCII zone map ──────────────────────────────────────────────────
// We'll plot a bounded chunk of Plane of Fear coordinates onto a fixed
// grid. Real coords range roughly y: 600..900, x: -1460..-1240.
const MAP_W = MAIN_W - 4;   // inside panel + 1 char gutter
const MAP_H = 22;
const MAP_X0 = -1470, MAP_X1 = -1230;
const MAP_Y0 = 620,   MAP_Y1 = 870;

function plot(x, y) {
  const col = Math.round(((x - MAP_X0) / (MAP_X1 - MAP_X0)) * (MAP_W - 1));
  const row = Math.round((1 - (y - MAP_Y0) / (MAP_Y1 - MAP_Y0)) * (MAP_H - 1));
  return { col, row };
}

function mapPanel() {
  // Build a 2D buffer of "cells" — each cell is { ch, color }.
  const grid = Array.from({ length: MAP_H }, () =>
    Array.from({ length: MAP_W }, () => ({ ch: " ", color: "muted" }))
  );

  // Terrain: faint dot field
  for (let r = 0; r < MAP_H; r++) {
    for (let c = 0; c < MAP_W; c++) {
      if ((r + c) % 6 === 0) grid[r][c] = { ch: "·", color: "dim" };
    }
  }

  // Rough zone outline — a scattered set of ridges
  const ridges = [
    [0.1, 0.5, 0.3, 0.5],
    [0.35, 0.25, 0.65, 0.25],
    [0.68, 0.4, 0.85, 0.55],
    [0.2, 0.75, 0.55, 0.75],
  ];
  for (const [x0, y0, x1, y1] of ridges) {
    const c0 = Math.round(x0 * (MAP_W - 1));
    const c1 = Math.round(x1 * (MAP_W - 1));
    const r0 = Math.round(y0 * (MAP_H - 1));
    const r1 = Math.round(y1 * (MAP_H - 1));
    const steps = Math.max(Math.abs(c1 - c0), Math.abs(r1 - r0));
    for (let s = 0; s <= steps; s++) {
      const t = s / steps;
      const cc = Math.round(c0 + (c1 - c0) * t);
      const rr = Math.round(r0 + (r1 - r0) * t);
      if (grid[rr] && grid[rr][cc]) grid[rr][cc] = { ch: "▒", color: "dim" };
    }
  }

  // Points of interest — zone lines, camp anchor
  const label = (r, c, text, color) => {
    for (let i = 0; i < text.length; i++) {
      if (grid[r] && grid[r][c + i]) grid[r][c + i] = { ch: text[i], color };
    }
  };
  label(1, 2, "⇱ Zone In", "cyan");
  label(1, MAP_W - 15, "Anchor: Fear Camp", "secondary");

  // Spawns
  for (const s of SPAWNS) {
    if (s.x < MAP_X0 || s.x > MAP_X1 || s.y < MAP_Y0 || s.y > MAP_Y1) continue;
    const { col, row } = plot(s.x, s.y);
    if (!grid[row] || !grid[row][col]) continue;
    if (s.type === "Named") grid[row][col] = { ch: "◆", color: "spawn-named" };
    else if (s.type === "Corpse") grid[row][col] = { ch: "†", color: "muted" };
    else grid[row][col] = { ch: "○", color: "spawn-npc" };
  }

  // PCs — use first letter of class, inverse color, only G2 (Fear)
  for (const c of CLIENTS) {
    if (c.group !== "G2") continue;
    const { col, row } = plot(c.pos.x, c.pos.y);
    if (!grid[row] || !grid[row][col]) continue;
    const ch = c.cls[0];
    grid[row][col] = { ch, color: c.cls === "CLR" ? "cyan" : c.cls === "WAR" ? "amber" : c.cls === "MAG" ? "magenta" : c.cls === "MNK" ? "red" : "bright", bold: true };
  }

  // Render rows
  const mapStr = grid
    .map((row) => {
      let out = "";
      let lastColor = null;
      let buf = "";
      for (const cell of row) {
        if (cell.color !== lastColor) {
          if (buf) out += `<span class="${lastColor}">${buf}</span>`;
          buf = "";
          lastColor = cell.color;
        }
        buf += cell.ch;
      }
      if (buf) out += `<span class="${lastColor}">${buf}</span>`;
      return out;
    })
    .join("\n");

  // We need each line wrapped in panel rows; split and feed
  const lines = mapStr.split("\n");
  const coordBand = [
    "",
    `<span class="muted">grid: Plane of Fear · y${MAP_Y0}..${MAP_Y1}, x${MAP_X0}..${MAP_X1} · resolution 1 cell ≈ ${((MAP_Y1 - MAP_Y0) / MAP_H).toFixed(1)}u</span>`,
    "",
    `<span class="secondary">Legend:</span>  <span class="spawn-named">◆</span> Named  ·  <span class="spawn-npc">○</span> NPC  ·  <span class="muted">†</span> Corpse  ·  <span class="cyan bold">C</span><span class="amber bold">W</span><span class="magenta bold">M</span><span class="red bold">K</span> <span class="secondary">clients</span>`,
  ];
  return panel([...lines, ...coordBand], {
    width: MAIN_W,
    title: `Tactical · Plane of Fear`,
    color: "magenta",
    foot: "hjkl pan  ·  +/- zoom  ·  f center on focus  ·  t target under cursor",
  });
}

// ─── Sidebar: spawn list, target, CH ─────────────────────────────────
function spawnList() {
  const lines = [];
  const grouped = {
    Named: SPAWNS.filter((s) => s.type === "Named"),
    NPC: SPAWNS.filter((s) => s.type === "NPC" && s.state !== "Dead"),
    Corpse: SPAWNS.filter((s) => s.type === "Corpse" || s.state === "Dead"),
  };

  const push = (label, arr, color) => {
    if (!arr.length) return;
    lines.push(`<span class="${color} bold">${label}</span> <span class="muted">(${arr.length})</span>`);
    for (const s of arr) {
      const hpStr = s.hp === 0 ? `<span class="muted">---</span>` : `<span class="${s.hp > 60 ? "hp-high" : s.hp > 30 ? "hp-mid" : "hp-low"}">${String(s.hp).padStart(3)}%</span>`;
      const name = pad(`<span class="${color}">${s.name}</span>`, 24);
      const lvl = `<span class="muted">L</span><span class="bright">${String(s.lvl).padStart(2)}</span>`;
      lines.push(`  ${name} ${lvl} ${hpStr}`);
    }
    lines.push("");
  };

  push("Named",  grouped.Named,  "spawn-named");
  push("NPCs",   grouped.NPC,    "spawn-npc");
  push("Corpses", grouped.Corpse, "muted");

  return panel(lines, { width: SIDEBAR_W, title: "Spawn List · 9 / 47 filtered", color: "magenta", foot: "/ filter  ·  n next named" });
}

function targetBox() {
  const target = { name: "Cazic-Thule", hp: 64, type: "Named", lvl: 66, cls: "SHD" };
  const lines = [
    `<span class="spawn-named bold">${target.name}</span> <span class="muted">· L${target.lvl} ${target.cls}</span>`,
    `<span class="secondary">HP</span> ${hpCell(target.hp, 24)}`,
    "",
    `<span class="secondary">Assisting</span>  <span class="highlight">${SESSION.mainAssist}</span>`,
    `<span class="secondary">Tanked by</span>  <span class="highlight">${SESSION.mainTank}</span>`,
    `<span class="secondary">On tank</span>    <span class="green">100%</span> agg  <span class="muted">· no add</span>`,
  ];
  return panel(lines, { width: SIDEBAR_W, title: `Target · Main Assist`, color: "cyan", titleColor: "bright" });
}

function chStatusBox() {
  const ch = SESSION.chChain;
  const lines = [
    `<span class="secondary">Members</span>   <span class="green bold">${ch.members} clerics</span>  <span class="muted">· ${ch.intervalSecs.toFixed(1)}s interval</span>`,
    `<span class="secondary">Adaptive</span>  <span class="${ch.adaptive ? "green" : "muted"}">${ch.adaptive ? "ON" : "off"}</span>`,
    `<span class="secondary">Target</span>    <span class="highlight">${SESSION.mainTank}</span>`,
    "",
    `<span class="muted">Slot 1</span>  <span class="cyan">Sylunariel</span>   <span class="bright">T+0.0s</span>  ${bar(0.62, 1, 12, { color: "cyan" })}`,
    `<span class="muted">Slot 2</span>  <span class="cyan">Aelwyn</span>       <span class="bright">T+1.7s</span>  ${bar(0.30, 1, 12, { color: "cyan" })}`,
    `<span class="muted">Slot 3</span>  <span class="cyan">Morrigaine</span>   <span class="bright">T+3.4s</span>  ${bar(0.05, 1, 12, { color: "cyan" })}`,
  ];
  return panel(lines, { width: SIDEBAR_W, title: "CH Chain · Active", color: "magenta" });
}

function composeTwoColumn(left, right) {
  const L = left.split("\n");
  const R = right.split("\n");
  const rows = Math.max(L.length, R.length);
  const blankL = " ".repeat(MAIN_W);
  const blankR = " ".repeat(SIDEBAR_W);
  const out = [];
  for (let i = 0; i < rows; i++) out.push((L[i] ?? blankL) + " " + (R[i] ?? blankR));
  return out.join("\n");
}

export function renderTactical() {
  const sidebar = [spawnList(), targetBox(), chStatusBox()].join("\n");
  return composeTwoColumn(mapPanel(), sidebar);
}
