// screen_orchestrator.js — Screen 7: Fleet ops board (slot lifecycle + signals).
import { ORCHESTRATOR } from "./data.js";
import { panel, pad, bar } from "./widgets.js";
import { W, MAIN_W, SIDEBAR_W } from "./shell.js";

function intentHeader() {
  const lines = [
    `<span class="secondary">Active Intent</span>   <span class="magenta bold">${ORCHESTRATOR.intent}</span>`,
    `<span class="secondary">Phase        </span>   <span class="${ORCHESTRATOR.phase === "Execute" ? "green" : "cyan"} bold">${ORCHESTRATOR.phase}</span>`,
    `<span class="secondary">Cadence      </span>   <span class="bright">${ORCHESTRATOR.cadence.toFixed(1)} Hz</span>   <span class="muted">tick ${ORCHESTRATOR.lastTick}</span>`,
  ];
  return panel(lines, { width: MAIN_W, title: "Fleet Orchestrator", color: "magenta", foot: "p pause  ·  r resume  ·  A abort intent  ·  enter drill into slot" });
}

function slotsTable() {
  const cols = [
    { label: "Slot", w: 5 },
    { label: "Name", w: 14 },
    { label: "State", w: 12 },
    { label: "FSM", w: 14 },
    { label: "Lat", w: 6, align: "right" },
    { label: "Health", w: 22 },
    { label: "Profile", w: 26 },
  ];
  const innerW = MAIN_W - 2;
  const header = cols.map((c) => pad(`<span class="secondary">${c.label}</span>`, c.w, c.align)).join("  ");
  const rows = [header, `<span class="dim">${"─".repeat(innerW)}</span>`];
  ORCHESTRATOR.slots.forEach((s) => {
    const stateCol =
      s.state === "Live" ? "green" :
      s.state === "Recovering" ? "amber" :
      s.state === "Blocked" ? "red" :
      s.state === "Configured" ? "cyan" : "muted";
    const health = s.state === "Live"
      ? `<span class="green">${"█".repeat(18)}</span> <span class="bright">OK</span>`
      : s.state === "Recovering"
      ? `<span class="amber">${"█".repeat(11)}</span><span class="dim">${"·".repeat(7)}</span> <span class="amber">RCV</span>`
      : s.state === "Blocked"
      ? `<span class="red">${"█".repeat(2)}</span><span class="dim">${"·".repeat(16)}</span> <span class="red">BLK</span>`
      : `<span class="dim">${"·".repeat(18)}</span> <span class="muted">OFF</span>`;
    rows.push(
      pad(`<span class="highlight">${s.slot}</span>`, 5) + "  " +
      pad(`<span class="bright">${s.name}</span>`, 14) + "  " +
      pad(`<span class="${stateCol} bold">${s.state}</span>`, 12) + "  " +
      pad(`<span class="cyan">${s.fsm}</span>`, 14) + "  " +
      pad(s.latencyMs ? `<span class="${s.latencyMs > 30 ? "amber" : "green"}">${s.latencyMs}ms</span>` : `<span class="muted">—</span>`, 6, "right") + "  " +
      pad(health, 22) + "  " +
      pad(`<span class="secondary">${s.profile}</span>`, 26)
    );
  });
  return panel(rows, { width: MAIN_W, title: "Slots · 6 live · 1 configured · 1 blocked", color: "magenta" });
}

function signalsFeed() {
  const lines = ORCHESTRATOR.signals.map((s) => {
    const kCol =
      s.kind === "CH" ? "cyan" :
      s.kind === "NAV" ? "amber" :
      s.kind === "ALERT" ? "red" :
      s.kind === "XP" ? "green" :
      s.kind === "LOOT" ? "magenta" :
      s.kind === "CAST" ? "cyan" : "bright";
    return `<span class="muted">${s.t}</span>  <span class="${kCol} bold">${pad(s.kind, 5)}</span>  <span class="bright">${s.msg}</span>`;
  });
  return panel(lines, { width: SIDEBAR_W, title: "Signal Feed", color: "cyan", titleColor: "bright" });
}

function phaseTimelineBox() {
  const lines = [
    `<span class="muted">11:42</span>  <span class="cyan">Initialize</span>   <span class="muted">── attach to fleet (6/6 slots)</span>`,
    `<span class="muted">11:43</span>  <span class="cyan">Plan</span>         <span class="muted">── loaded ch-chain.alpha</span>`,
    `<span class="muted">11:44</span>  <span class="cyan">Prepare</span>      <span class="muted">── nav to fear.camp (all)</span>`,
    `<span class="muted">11:52</span>  <span class="green">Execute</span>      <span class="bright">── running (2h 12m)</span>`,
    "",
    `<span class="secondary">next: </span><span class="magenta">Checkpoint</span> <span class="muted">at kills=150</span>`,
  ];
  return panel(lines, { width: SIDEBAR_W, title: "Phase Timeline", color: "magenta" });
}

function composeTwo(left, right) {
  const L = left.split("\n"), R = right.split("\n");
  const h = Math.max(L.length, R.length);
  const out = [];
  for (let i = 0; i < h; i++) out.push((L[i] ?? " ".repeat(MAIN_W)) + " " + (R[i] ?? " ".repeat(SIDEBAR_W)));
  return out.join("\n");
}

export function renderOrchestrator() {
  const main = intentHeader() + "\n" + slotsTable();
  const right = signalsFeed() + "\n" + phaseTimelineBox();
  return composeTwo(main, right);
}
