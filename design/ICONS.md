# Icon System — TextQuest

**Library:** [lucide-react](https://lucide.dev) — Apache-2.0, tree-shakeable SVG icons.

**Why Lucide over Phosphor/Tabler/Heroicons:**

- React-native (no wrapper). Each icon = named export, Vite tree-shakes to only what's imported.
- Stroke-based linework matches angular/spaced Neriak aesthetic (no cute filled glyphs).
- Covers operator-dashboard vocabulary: `Activity`, `Server`, `Terminal`, `Zap`, `Shield`, `Users`, `Target`, `AlertTriangle`.
- `strokeWidth` prop lets us match ratatui box-drawing weight (1.5–2).

## Install

```bash
cd textquest-web/frontend
pnpm add lucide-react
```

(Already added to `package.json` dependencies.)

## Usage

```tsx
import { Activity, Server, AlertTriangle } from "lucide-react";

<Activity className="w-4 h-4 text-neriak-cyan" strokeWidth={1.75} />;
```

**Conventions:**

- Default size: `w-4 h-4` (16px) for inline labels, `w-5 h-5` (20px) for card headers, `w-6 h-6` (24px) for nav.
- Default strokeWidth: `1.75`. Ratatui parity.
- Color via Tailwind `text-*` utilities mapped to token colors (`text-neriak-cyan`, `text-state-ok`, etc.). Never hardcode.
- Never wrap in `<span>` just for spacing — use flex gap.

## Semantic icon map

Keep this list stable. When adding new features, extend here before scattering icons.

| Concept          | Icon              | Color token                  |
| ---------------- | ----------------- | ---------------------------- |
| Clients          | `Users`           | `text-neriak-cyan`           |
| Groups           | `UsersRound`      | `text-state-ok`              |
| Economy / Krono  | `Coins`           | `text-state-warn`            |
| Server / session | `Server`          | `text-neriak-magenta-bright` |
| Healthy          | `CheckCircle2`    | `text-state-ok`              |
| Warning          | `AlertTriangle`   | `text-state-warn`            |
| Blocked / error  | `XCircle`         | `text-state-danger`          |
| Info             | `Info`            | `text-state-info`            |
| Combat           | `Swords`          | `text-neriak-magenta`        |
| Navigation       | `Map` / `Compass` | `text-neriak-cyan`           |
| Login chain      | `KeyRound`        | `text-neriak-magenta`        |
| Packet capture   | `Activity`        | `text-state-info`            |
| Terminal / logs  | `Terminal`        | `text-neriak-muted`          |
| Settings         | `Settings`        | `text-neriak-muted`          |
| Command palette  | `Command`         | `text-neriak-cyan`           |
| Privacy mode     | `EyeOff`          | `text-neriak-muted`          |
| Danger zone      | `ShieldAlert`     | `text-state-danger`          |

## Do not

- Mix icon libraries. One library = consistent stroke weight, sizing, keyline grid.
- Import from `lucide-react/dist/esm/icons/*` — breaks tree-shaking. Use named imports only.
- Add new colors just for icons. Colors come from `tokens.json` only.
- Use filled/emoji icons anywhere. Linework only.
