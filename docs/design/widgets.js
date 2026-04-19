// widgets.js — box-drawing panel helpers, bars, and misc TUI primitives.
//
// All panels render as <pre>-style text: the visual border is literal
// Unicode box characters, not CSS borders. This matches how the real app
// draws with ratatui's Block widget.

export const BOX = {
  // Thin rounded — primary
  rounded: { tl: "╭", tr: "╮", bl: "╰", br: "╯", h: "─", v: "│", tLeft: "├", tRight: "┤", tUp: "┴", tDown: "┬", cross: "┼" },
  // Double — server / strong emphasis
  double: { tl: "╔", tr: "╗", bl: "╚", br: "╝", h: "═", v: "║", tLeft: "╠", tRight: "╣", tUp: "╩", tDown: "╦", cross: "╬" },
  // Thick — active / highlight
  thick: { tl: "┏", tr: "┓", bl: "┗", br: "┛", h: "━", v: "┃", tLeft: "┣", tRight: "┫", tUp: "┻", tDown: "┳", cross: "╋" },
};

// Strip HTML tags when counting visible width. We build rows with inline
// <span> for color — the width math must ignore the tags.
export function vlen(s) {
  return String(s).replace(/<[^>]*>/g, "").length;
}

// Pad a (possibly-tagged) string to a visible width.
export function pad(s, width, align = "left") {
  const n = vlen(s);
  if (n >= width) return s;
  const diff = width - n;
  if (align === "right") return " ".repeat(diff) + s;
  if (align === "center") {
    const l = Math.floor(diff / 2);
    return " ".repeat(l) + s + " ".repeat(diff - l);
  }
  return s + " ".repeat(diff);
}

export function trunc(s, width) {
  const plain = String(s).replace(/<[^>]*>/g, "");
  if (plain.length <= width) return s;
  // Hard-truncate the plain form — cheap and fine for our mock
  return plain.slice(0, width - 1) + "…";
}

// Build a top/bottom border with an optional title inset on the top.
// style: "rounded" | "double" | "thick"
// color: CSS class for the border characters
// title: plain string; its characters appear at their natural color, but
//        the surrounding border stays in `color`.
export function borderTop(width, title, style = "rounded", color = "magenta", titleColor = "bright") {
  const b = BOX[style];
  if (!title) {
    return `<span class="${color}">${b.tl}${b.h.repeat(width - 2)}${b.tr}</span>`;
  }
  const lead = 2;
  const label = ` ${title} `;
  const fill = width - 2 - lead - label.length;
  return (
    `<span class="${color}">${b.tl}${b.h.repeat(lead)}</span>` +
    `<span class="${titleColor}">${label}</span>` +
    `<span class="${color}">${b.h.repeat(Math.max(0, fill))}${b.tr}</span>`
  );
}

export function borderBottom(width, style = "rounded", color = "magenta", foot) {
  const b = BOX[style];
  if (!foot) {
    return `<span class="${color}">${b.bl}${b.h.repeat(width - 2)}${b.br}</span>`;
  }
  const lead = 2;
  const label = ` ${foot} `;
  const fill = width - 2 - lead - label.length;
  return (
    `<span class="${color}">${b.bl}${b.h.repeat(lead)}</span>` +
    `<span class="muted">${label}</span>` +
    `<span class="${color}">${b.h.repeat(Math.max(0, fill))}${b.br}</span>`
  );
}

export function borderMid(width, style = "rounded", color = "magenta") {
  const b = BOX[style];
  return `<span class="${color}">${b.tLeft}${b.h.repeat(width - 2)}${b.tRight}</span>`;
}

// A row inside a panel — pads content to `width - 2` and adds side walls.
export function row(content, width, style = "rounded", color = "magenta") {
  const b = BOX[style];
  return (
    `<span class="${color}">${b.v}</span>` +
    pad(content, width - 2) +
    `<span class="${color}">${b.v}</span>`
  );
}

// Build a whole panel from an array of inner rows.
export function panel(innerRows, { width, title, style = "rounded", color = "magenta", titleColor = "bright", foot } = {}) {
  const lines = [borderTop(width, title, style, color, titleColor)];
  for (const r of innerRows) lines.push(row(r, width, style, color));
  lines.push(borderBottom(width, style, color, foot));
  return lines.join("\n");
}

// Progress bar in a fixed char width. Returns colored block chars.
// Bar chars: █ ▉ ▊ ▋ ▌ ▍ ▎ ▏
export function bar(value, max, width, opts = {}) {
  const pct = Math.max(0, Math.min(1, max ? value / max : 0));
  const filled = Math.floor(pct * width);
  const partial = pct * width - filled;
  const steps = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];
  const stepIdx = Math.min(7, Math.floor(partial * 8));
  const head = steps[stepIdx];

  let color = opts.color;
  if (!color) {
    if (opts.hp) {
      color = pct > 0.6 ? "hp-high" : pct > 0.3 ? "hp-mid" : "hp-low";
    } else if (opts.mana) {
      color = "mana";
    } else {
      color = "accent";
    }
  }
  const full = "█".repeat(filled);
  const empty = "·".repeat(Math.max(0, width - filled - (head ? 1 : 0)));
  return `<span class="${color}">${full}${head}</span><span class="dim">${empty}</span>`;
}

// HP cell: `96% ████▌···` in HP color
export function hpCell(pct, barWidth = 8) {
  const cls = pct > 60 ? "hp-high" : pct > 30 ? "hp-mid" : "hp-low";
  const label = `${String(pct).padStart(3)}%`;
  return `<span class="${cls}">${label}</span> ${bar(pct, 100, barWidth, { hp: true })}`;
}

// Small pill badge, inverse-colored
export function pill(label, color = "inv-magenta") {
  return `<span class="${color}"> ${label} </span>`;
}

// Multi-column header row — accepts [{label,width,align,cls}]
export function headerRow(cols, { sep = " " } = {}) {
  return cols
    .map((c) => {
      const s = `<span class="${c.cls || "secondary"}">${c.label}</span>`;
      return pad(s, c.width, c.align || "left");
    })
    .join(sep);
}

// Same, for data rows.
export function dataRow(cols, { sep = " " } = {}) {
  return cols
    .map((c) => pad(c.v, c.width, c.align || "left"))
    .join(sep);
}
