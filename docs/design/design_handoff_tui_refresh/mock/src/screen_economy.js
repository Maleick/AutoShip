// screen_economy.js — Screen 6: Vendor / Bank cycle controls.
import { ECONOMY } from "./data.js";
import { panel, pad, bar } from "./widgets.js";
import { W, MAIN_W, SIDEBAR_W } from "./shell.js";

function cycleStatus() {
  const lines = [
    `<span class="secondary">Cycle State</span>   <span class="green bold">${ECONOMY.state}</span> · <span class="bright">${ECONOMY.stage}</span>`,
    `<span class="secondary">Current Slot</span>  <span class="highlight bold">${ECONOMY.currentSlot}</span>`,
    `<span class="secondary">Stage Started</span> <span class="bright">${ECONOMY.stageStarted}</span>`,
    `<span class="secondary">Cycle Started</span> <span class="bright">${ECONOMY.cycleStarted}</span>`,
    `<span class="secondary">Cycles Today</span>  <span class="bright">${ECONOMY.cyclesToday}</span>`,
    `<span class="secondary">Next Cycle In</span> <span class="cyan">${ECONOMY.nextCycleIn}</span>`,
    "",
    `<span class="secondary">Vendor</span>  <span class="magenta">${ECONOMY.vendor}</span>`,
    `<span class="secondary">Bank  </span>  <span class="magenta">${ECONOMY.bank}</span>`,
    "",
    `<span class="secondary">Plat (pocket)</span>  <span class="bright bold">${ECONOMY.plat.toFixed(1)}</span>`,
    `<span class="secondary">Plat (banked)</span>  <span class="bright">${ECONOMY.bankedPlat.toLocaleString()}</span>`,
  ];
  return panel(lines, { width: MAIN_W, title: "Vendor / Bank Cycle", color: "magenta", foot: "s start  ·  S stop  ·  x skip client  ·  r reload rules" });
}

function rosterTable() {
  const cols = [
    { label: "Slot", w: 16 },
    { label: "Status", w: 10 },
    { label: "Plat", w: 6, align: "right" },
    { label: "Bags", w: 6 },
    { label: "Reason / Notes", w: 34 },
  ];
  const innerW = MAIN_W - 2;
  const header = cols.map((c) => pad(`<span class="secondary">${c.label}</span>`, c.w, c.align)).join("  ");
  const rows = [header, `<span class="dim">${"─".repeat(innerW)}</span>`];
  ECONOMY.roster.forEach((r) => {
    const statusCol =
      r.status === "Active" ? "amber" :
      r.status === "Done" ? "green" :
      r.status === "Queued" ? "cyan" :
      r.status === "Skipped" ? "muted" : "bright";
    const nameCol = r.status === "Active" ? "inv-amber" : "bright";
    const name = r.status === "Active" ? `<span class="inv-amber"> ${pad(r.name, 14)} </span>` : `<span class="${nameCol}">${pad(r.name, 16)}</span>`;
    const notes = r.reason ? `<span class="muted">skip: </span><span class="amber">${r.reason}</span>` : r.status === "Done" ? `<span class="muted">cycle complete · bagged 3 lore items</span>` : r.status === "Active" ? `<span class="cyan">selling 14 items · 20% through</span>` : `<span class="muted">waiting in queue</span>`;
    rows.push(
      name + "  " +
      pad(`<span class="${statusCol}">${r.status}</span>`, 10) + "  " +
      pad(`<span class="bright">${r.plat}</span>`, 6, "right") + "  " +
      pad(`<span class="secondary">${r.bags}</span>`, 6) + "  " +
      pad(notes, 34)
    );
  });
  return panel(rows, { width: MAIN_W, title: "Roster", color: "magenta" });
}

function rulesBox() {
  const lines = ECONOMY.rules.map((r, i) => `  <span class="cyan">${i + 1}.</span> <span class="bright">${r}</span>`);
  lines.unshift(`<span class="secondary">active rule set</span> <span class="bright">default.ron</span>  <span class="muted">(4 rules)</span>`);
  lines.push("");
  lines.push(`<span class="muted">rules load from ~/.config/textquest/economy/</span>`);
  lines.push(`<span class="muted">press e to edit · r to reload · t to test</span>`);
  return panel(lines, { width: SIDEBAR_W, title: "Rules", color: "cyan", titleColor: "bright" });
}

function ledgerBox() {
  const lines = [
    `<span class="secondary">Today</span>`,
    `  <span class="secondary">Earned </span>  <span class="green">+2,418 pp</span>`,
    `  <span class="secondary">Vendor </span>  <span class="cyan">+1,902 pp</span>`,
    `  <span class="secondary">Loot   </span>  <span class="amber">+516 pp</span>`,
    "",
    `<span class="secondary">Last Cycle</span>`,
    `  <span class="secondary">Sold   </span>  <span class="bright">42 items</span>`,
    `  <span class="secondary">Banked </span>  <span class="bright">12,480 pp</span>`,
    `  <span class="secondary">Skipped</span>  <span class="amber">2 (combat)</span>`,
  ];
  return panel(lines, { width: SIDEBAR_W, title: "Ledger", color: "magenta" });
}

function composeTwo(left, right) {
  const L = left.split("\n"), R = right.split("\n");
  const h = Math.max(L.length, R.length);
  const out = [];
  for (let i = 0; i < h; i++) out.push((L[i] ?? " ".repeat(MAIN_W)) + " " + (R[i] ?? " ".repeat(SIDEBAR_W)));
  return out.join("\n");
}

export function renderEconomy() {
  const main = cycleStatus() + "\n" + rosterTable();
  const right = rulesBox() + "\n" + ledgerBox();
  return composeTwo(main, right);
}
