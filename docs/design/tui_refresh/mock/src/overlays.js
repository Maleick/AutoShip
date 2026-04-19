// overlays.js — Help, Command, Config, CH-Chain, Wizard, Menu bar, Alert feed, Toast.
import { ALERTS, MENUBAR, SESSION, CLIENTS } from "./data.js";
import { panel, pad } from "./widgets.js";
import { W } from "./shell.js";

// Centered width for dialog overlays
const DLG_W = 96;

export function renderHelp() {
  const sections = [
    ["Navigation", [
      ["1-7",        "Jump to screen (Char / Map / Nav / Dbg / Pkt / Eco / Orc)"],
      ["Tab / ⇧Tab", "Cycle pane within current screen"],
      ["[ / ]",      "Previous / next client"],
      ["g",          "Cycle focus group"],
      ["F10",        "Open menu bar"],
    ]],
    ["Overlays", [
      ["?",  "Toggle this help"],
      [":",  "Command mode (:assist, :pull, :ch, :mode, :nav)"],
      ["F2", "Open config tree"],
      ["F3", "Open CH-chain panel"],
      ["F8", "Toggle alert feed"],
      ["F4", "Launch fleet wizard"],
    ]],
    ["Combat / Ops", [
      ["a",  "Assist main assist"],
      ["p",  "Pull current target"],
      ["e / d", "Engage all / disengage"],
      ["c / s", "Start / stop CH chain"],
      ["l",  "Loot all corpses in range"],
    ]],
    ["Roster / Debug", [
      ["↑ ↓", "Select row"],
      ["/",  "Filter current list"],
      ["t",  "Set target from selection"],
      ["i",  "Inspect raw packet / spawn"],
      ["m",  "Toggle Camp ↔ Hunt mode"],
    ]],
  ];
  const lines = [];
  for (const [title, pairs] of sections) {
    lines.push(`<span class="magenta bold">${title}</span>`);
    for (const [k, v] of pairs) {
      lines.push(`  <span class="inv-cyan"> ${pad(k, 8)} </span>  <span class="bright">${v}</span>`);
    }
    lines.push("");
  }
  lines.pop();
  return panel(lines, { width: DLG_W, title: "Help · Keybinds (press ? to close)", color: "cyan", titleColor: "bright" });
}

export function renderCommand() {
  const lines = [
    `<span class="secondary">suggestions</span>`,
    `  <span class="cyan">:assist</span>         <span class="muted">assist the main assist</span>`,
    `  <span class="cyan">:pull [target]</span>  <span class="muted">send puller to target or selected</span>`,
    `  <span class="cyan">:ch start|stop|adaptive on|off</span>`,
    `  <span class="cyan">:mode camp|hunt</span>`,
    `  <span class="cyan">:nav &lt;dest&gt;</span>  <span class="muted">move all / focus group to named location</span>`,
    `  <span class="cyan">:vendor run|skip|abort</span>`,
    "",
    `<span class="magenta bold">: </span><span class="bright">nav fear.ch_anchor focus=G2</span><span class="magenta">█</span>`,
  ];
  return panel(lines, { width: DLG_W, title: "Command mode", color: "magenta" });
}

export function renderConfig() {
  const tree = [
    [0, "▾", "textquest", "bright"],
    [1, "▾", "fleet", "cyan"],
    [2, "·", "roster_path", "secondary", "/home/op/eq/roster.ron"],
    [2, "·", "profile_dir", "secondary", "~/.config/textquest/profiles"],
    [1, "▾", "combat", "cyan"],
    [2, "·", "ch_chain.members", "secondary", "3"],
    [2, "·", "ch_chain.interval_secs", "secondary", "5.0"],
    [2, "▸", "ch_chain.adaptive", "green", "true"],
    [2, "·", "engage_on_assist", "secondary", "true"],
    [1, "▾", "nav", "cyan"],
    [2, "·", "mesh_file", "secondary", "fear.mesh"],
    [2, "·", "stuck_retry_limit", "secondary", "5"],
    [1, "▾", "economy", "cyan"],
    [2, "·", "rules_file", "secondary", "default.ron"],
    [2, "·", "cycle_interval", "secondary", "3600s"],
    [1, "▾", "ui", "cyan"],
    [2, "·", "theme", "secondary", "neriak"],
    [2, "·", "width_class", "secondary", "auto"],
    [2, "▸", "privacy_mode", "amber", "false"],
  ];
  const lines = tree.map(([depth, glyph, key, col, val]) => {
    const indent = "  ".repeat(depth);
    const g = `<span class="magenta">${glyph}</span>`;
    const k = `<span class="${col}">${key}</span>`;
    const v = val ? ` <span class="muted">=</span> <span class="bright">${val}</span>` : "";
    return `${indent}${g} ${k}${v}`;
  });
  return panel(lines, { width: DLG_W, title: "Config · ~/.config/textquest/config.ron", color: "cyan", titleColor: "bright", foot: "↑↓ navigate  ·  enter edit  ·  s save  ·  r reload" });
}

export function renderChChain() {
  const lines = [
    `<span class="secondary">Status  </span>  <span class="green bold">Active</span>   <span class="secondary">Adaptive</span>  <span class="green">ON</span>`,
    `<span class="secondary">Members </span>  <span class="bright">3 clerics</span> <span class="muted">·</span> interval <span class="bright">5.0s</span>`,
    `<span class="secondary">Chained </span>  target <span class="highlight">${SESSION.mainTank}</span>`,
    "",
    `<span class="bright bold">Slot schedule</span>`,
    `  <span class="cyan">Sylunariel</span>   T+0.0s   <span class="muted">CH #142</span>  <span class="green">✓ 12:04:16.5</span>`,
    `  <span class="cyan">Aelwyn</span>       T+1.7s   <span class="muted">CH #143</span>  <span class="amber">◷ 0.6s out</span>`,
    `  <span class="cyan">Morrigaine</span>   T+3.4s   <span class="muted">CH #144</span>  <span class="dim">queued</span>`,
    "",
    `<span class="secondary">Adaptive</span> expanded interval 4.8s → 5.0s on last safety check.`,
    `<span class="muted">Configured failsafes: resist-retry, OOM-fallback, tank-death abort.</span>`,
  ];
  return panel(lines, { width: DLG_W, title: "CH Chain · panel", color: "magenta" });
}

export function renderWizard() {
  const lines = [
    `<span class="cyan bold">Step 2 of 5</span> · <span class="bright">Choose fleet profile</span>`,
    "",
    `  <span class="magenta">◉</span> <span class="bright">6-box Cleric Core</span>     <span class="muted">CH chain, MT war, 1 puller, 1 mez, 2 DPS</span>`,
    `  <span class="muted">○</span> <span class="secondary">Molo Pair</span>              <span class="muted">necro + chanter, light automation</span>`,
    `  <span class="muted">○</span> <span class="secondary">Raid Cleric (12)</span>       <span class="muted">heavy CH rotation, no tanks</span>`,
    `  <span class="muted">○</span> <span class="secondary">Vendor Mule</span>             <span class="muted">economy-only, no combat</span>`,
    `  <span class="muted">○</span> <span class="secondary">Custom...</span>              <span class="muted">blank slate</span>`,
    "",
    `<span class="muted">──────────────────────────────────────────────────────────────────────────────────────────</span>`,
    ``,
    `<span class="secondary">Zone  </span>  <span class="bright">Plane of Fear</span>`,
    `<span class="secondary">Server</span>  <span class="server">Bertoxxulous</span>`,
    `<span class="secondary">Slots </span>  <span class="bright">6</span>  <span class="muted">(drawn from roster.ron — edit with F2)</span>`,
    "",
    `  <span class="inv-magenta"> ◀ Back </span>   <span class="inv-cyan"> Next ▶ </span>   <span class="muted">esc cancel</span>`,
  ];
  return panel(lines, { width: DLG_W, title: "Fleet Setup · wizard", color: "magenta" });
}

export function renderAlertFeed() {
  const lines = [];
  ALERTS.forEach((a, i) => {
    const sevCol = a.sev === "Critical" ? "red" : a.sev === "Warning" ? "amber" : "cyan";
    const mark = a.ack ? `<span class="muted">○</span>` : `<span class="${sevCol} bold">●</span>`;
    lines.push(`${mark} <span class="${sevCol} bold">${pad(a.sev, 9)}</span> <span class="secondary">${a.when}</span>  <span class="bright">${a.msg}</span>`);
    lines.push(`  <span class="muted">kind=${a.kind} source=${a.source}</span>`);
    if (i < ALERTS.length - 1) lines.push("");
  });
  return panel(lines, { width: DLG_W, title: `Alert Feed · ${ALERTS.filter((a) => !a.ack).length} unread`, color: "amber", foot: "↑↓ select  ·  a acknowledge  ·  A ack all  ·  F8 close" });
}

export function renderMenuBar() {
  const b = "─";
  const lines = [];
  const bar = MENUBAR.bar
    .map((m) => m === MENUBAR.active
      ? `<span class="inv-magenta"> ${m} </span>`
      : `<span class="secondary"> ${m} </span>`)
    .join(" ");
  lines.push(bar);
  lines.push(`<span class="dim">${"─".repeat(DLG_W - 2)}</span>`);
  MENUBAR.items.forEach((it) => {
    if (it.key === "—") {
      lines.push(`<span class="dim">${"─".repeat(DLG_W - 2)}</span>`);
    } else {
      const left = `  <span class="magenta bold">${it.key}</span>  <span class="bright">${it.label}</span>`;
      const right = `<span class="muted">${it.hint}</span>`;
      const fill = Math.max(1, DLG_W - 2 - (left.replace(/<[^>]*>/g, "").length) - (right.replace(/<[^>]*>/g, "").length));
      lines.push(left + " ".repeat(fill) + right);
    }
  });
  return panel(lines, { width: DLG_W, title: "Menu · F10", color: "cyan", titleColor: "bright" });
}

export function renderToast() {
  const lines = [
    `<span class="green bold">●</span> <span class="bright bold">CH #142 landed</span> on <span class="highlight">Thurgrek</span>`,
    `  <span class="muted">Sylunariel · Complete Healing · 10,000 hp · 12:04:16.5</span>`,
  ];
  return panel(lines, { width: 62, title: "Toast", color: "green" });
}
