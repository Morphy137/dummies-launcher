---
version: alpha
name: Dummies
status: draft
description: A dark-first, Raycast-inspired desktop system for a lightweight game-presence launcher. Near-black canvas, four-step surface ladder, 1px hairlines, Inter with ss03, a single high-contrast CTA, and one red diagonal-stripe moment. Fluent 2 supplies interaction, accessibility, and motion behavior; the visual language stays restrained and product-like.
website: "https://github.com/Morphy137/dummies-launcher"
tags:
  - "Desktop"
  - "Productivity & SaaS"
  - "Gaming"
lastUpdated: "2026-10-21"
---

# Dummies Design System

Implementation note (2026-10-25): this is a visual reference, not a feature checklist.
`ARCHITECTURE.md` is authoritative for shipped behavior. Settings is a modal with explicit
Save/Cancel; Escape/backdrop cancels. Catalog results use bounded pages rather than an
unbounded virtualized command palette. Native materials, generated bindings and a native
renderer remain out of scope. Timer pause affects only the local estimate.

This is an adaptation of the Raycast DESIGN.md visual language for a Windows desktop
launcher. It is not a copy of Raycast's product UI. The app keeps Raycast's restraint:
near-black surfaces, hairline borders, no drop shadows, Inter with the `ss03` stylistic
set, a single high-contrast primary action, and one chromatic stripe moment.

Fluent 2 is used for behavior and accessibility: focus management, dialogs, menus,
switches, tooltips, toasts, and motion. The Fluent visual defaults (shadows, blue
accents, rounded web cards) are overridden by the tokens below.

## Principles

1. **The app is the product surface.** No marketing chrome, no dashboard cards, no KPI
   tiles. Every screen should feel like an extended command palette.
2. **One primary action per screen.** Dark mode uses a white primary button. Light
   mode uses a black primary button. There is no second primary color.
3. **Depth comes from surfaces, not shadows.** Use the four-step surface ladder and
   1px hairlines. Do not add drop shadows to cards, menus, or dialogs.
4. **Color is information.** Saturated accents are reserved for status, category,
   focus, and progress. They never become the main button color.
5. **Art carries the emotion.** Game covers are the only large color fields. The chrome
   stays monochrome so the library can breathe.
6. **Motion clarifies, never decorates.** Short fades and transforms only. Respect
   Windows animation and transparency preferences.
7. **Native where it matters.** Window controls, snap layouts, tray menus, file paths,
   notifications, and keyboard behavior should feel like Windows, even when the
   content uses a custom visual language.

## Modes and materials

- **Dark is canonical.** The default appearance is the near-black Raycast surface.
- **Light is an adaptation.** It mirrors the same hierarchy and hairlines with an
  inverted neutral ladder. It is not a literal Raycast theme.
- **System theme by default.** The user can choose System, Dark, or Light.
- **Material: Solid by default.** This preserves the pure near-black canvas.
- **Material: Mica (Windows 11) is optional.** Apply it only to the title bar and
  window backdrop; content surfaces stay opaque. On Windows 10, Mica is unavailable.
- **Acrylic is optional and transient.** Use it only for the command palette, dialogs,
  and menus. Never apply Acrylic to the main content surface.
- **Transparency effects off or high contrast:** force Solid.
- **Reduced motion:** disable transforms and shared-element transitions; keep short
  opacity changes only.

## Tokens

### Color — dark

```yaml
colors:
  dark:
    canvas: "#07080a"
    surface: "#0d0d0d"
    surface-elevated: "#101111"
    surface-card: "#121212"
    surface-hover: "#151617"
    hairline: "#242728"
    hairline-soft: "rgba(255,255,255,0.08)"
    hairline-strong: "rgba(255,255,255,0.16)"
    text-primary: "#f4f4f6"
    text-body: "#cdcdcd"
    text-muted: "#9c9c9d"
    text-faint: "#6a6b6c"
    text-disabled: "#434345"
    primary: "#ffffff"
    primary-hover: "#e8e8e8"
    on-primary: "#000000"
    overlay: "rgba(0,0,0,0.64)"
    selection: "rgba(87,193,255,0.16)"
    focus: "var(--system-accent, #57c1ff)"
    accent-blue: "#57c1ff"
    accent-blue-soft: "rgba(87,193,255,0.15)"
    accent-red: "#ff6161"
    accent-red-soft: "rgba(255,97,97,0.15)"
    accent-green: "#59d499"
    accent-green-soft: "rgba(89,212,153,0.15)"
    accent-yellow: "#ffc533"
    accent-yellow-soft: "rgba(255,197,51,0.15)"
    stripe-start: "#ff5757"
    stripe-end: "#a1131a"
```

### Color — light adaptation

```yaml
colors:
  light:
    canvas: "#f6f6f7"
    surface: "#ffffff"
    surface-elevated: "#fbfbfc"
    surface-card: "#ffffff"
    surface-hover: "#f0f0f2"
    hairline: "#e4e4e7"
    hairline-soft: "rgba(0,0,0,0.06)"
    hairline-strong: "rgba(0,0,0,0.12)"
    text-primary: "#18191a"
    text-body: "#3f3f46"
    text-muted: "#71717a"
    text-faint: "#a1a1aa"
    text-disabled: "#d4d4d8"
    primary: "#111111"
    primary-hover: "#2a2a2a"
    on-primary: "#ffffff"
    overlay: "rgba(0,0,0,0.32)"
    selection: "rgba(11,132,214,0.12)"
    focus: "var(--system-accent, #0b84d6)"
    accent-blue: "#0b84d6"
    accent-blue-soft: "rgba(11,132,214,0.12)"
    accent-red: "#d92d2d"
    accent-red-soft: "rgba(217,45,45,0.10)"
    accent-green: "#0f8a5f"
    accent-green-soft: "rgba(15,138,95,0.10)"
    accent-yellow: "#9a5b00"
    accent-yellow-soft: "rgba(255,197,51,0.18)"
    stripe-start: "#ff5757"
    stripe-end: "#a1131a"
```

### Typography

Inter Variable, bundled locally. Apply the stylistic set globally:

```css
font-feature-settings: "calt", "kern", "liga", "ss03";
```

Fallback: `Segoe UI Variable`, `Segoe UI`, sans-serif. Monospace: `Cascadia Code`,
`Cascadia Mono`, `Consolas`.

```yaml
typography:
  display-xl:
    fontFamily: Inter
    fontSize: 56px
    fontWeight: 600
    lineHeight: 1.05
    letterSpacing: -0.8px
  display-lg:
    fontFamily: Inter
    fontSize: 44px
    fontWeight: 600
    lineHeight: 1.08
    letterSpacing: -0.6px
  heading-xl:
    fontFamily: Inter
    fontSize: 28px
    fontWeight: 500
    lineHeight: 1.2
    letterSpacing: -0.2px
  heading-lg:
    fontFamily: Inter
    fontSize: 24px
    fontWeight: 500
    lineHeight: 1.25
    letterSpacing: -0.1px
  heading-md:
    fontFamily: Inter
    fontSize: 20px
    fontWeight: 500
    lineHeight: 1.4
    letterSpacing: 0
  heading-sm:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: 500
    lineHeight: 1.4
    letterSpacing: 0.1px
  body-lg:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: 400
    lineHeight: 1.6
    letterSpacing: 0
  body-md:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: 400
    lineHeight: 1.6
    letterSpacing: 0
  body-sm:
    fontFamily: Inter
    fontSize: 13px
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: 0.1px
  body-strong:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: 500
    lineHeight: 1.4
    letterSpacing: 0.1px
  caption:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: 0.4px
  button:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: 500
    lineHeight: 1.2
    letterSpacing: 0.1px
  mono:
    fontFamily: Cascadia Code
    fontSize: 12px
    fontWeight: 400
    lineHeight: 1.6
```

### Radius, spacing, motion

```yaml
rounded:
  none: 0px
  xs: 4px
  sm: 6px
  md: 8px
  lg: 10px
  xl: 16px
  full: 9999px

spacing:
  xxs: 2px
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 24px
  xxl: 32px
  section: 48px
  hero: 96px

motion:
  instant: 0ms
  fast: 120ms
  normal: 180ms
  slow: 240ms
  page: 300ms
  easing-standard: "cubic-bezier(0.2, 0, 0, 1)"
  easing-exit: "cubic-bezier(0.4, 0, 1, 1)"
```

App sections use 32px or 48px. Reserve 96px for the empty state or a single hero
moment. Do not apply 96px rhythm to every panel.

## Component specifications

| Component | Surface | Border | Radius | Height | Notes |
|---|---|---|---|---|---|
| App title bar | canvas | hairline bottom | 0 | 48px | Custom drag region; native-like caption buttons |
| Command bar | canvas | none | 0 | auto | Search, filter pills, Add game CTA |
| Search input | surface-elevated | hairline | md | 36–44px | Leading search icon; focus ring in system accent |
| Primary button | primary | none | md | 36px | White button in dark; black button in light; black/white label |
| Secondary button | transparent | hairline | md | 36px | Text primary; hover surface-elevated |
| Ghost button | transparent | none | md | 32–36px | Text body; hover surface-hover |
| Filter pill | transparent | none | full | 28px | Active: surface-elevated + text primary |
| Poster card | surface | hairline | lg | auto | 2:3 art; no shadow; hover surface-card |
| Poster overlay | overlay | none | lg | auto | Play and favorite actions only |
| Running badge | accent-green-soft | none | xs | 20px | Green text and dot; timer in tabular numerals |
| Quest dock | surface | hairline | lg | auto | Progress bar, phase, remaining time |
| Command palette | surface | hairline | xl | auto | Max 720px; header input; virtualized rows |
| Command row | transparent | none | sm | 40px | Active/hover surface-card; keycap on the right |
| Settings row | canvas | hairline bottom | 0 | auto | Label + description + control; no card nesting |
| Activity drawer | surface | hairline top | 0 | 180–260px | Monospace log; slides from bottom |
| Dialog | surface | hairline | lg | auto | Overlay dim; no drop shadow |
| Toast | surface-elevated | hairline | lg | auto | Accent icon; 12px caption; top-right |
| Keycap | surface-card | hairline | xs | 20px | 12px mono/caption |
| Red stripe | gradient | none | 0 | 3px | One per screen maximum; top edge or empty-state band |

### Buttons

- Primary: `background: primary; color: on-primary; border-radius: 8px; height: 36px;
  padding: 8px 16px; font: button;` Hover uses `primary-hover`. No shadow.
- Secondary: transparent with `1px solid hairline`; hover `surface-elevated`.
- Destructive: transparent with `1px solid accent-red`; text `accent-red`; hover
  `accent-red-soft`. The destructive action is never the default focus.
- Icon button: 32×32 or 36×36; hit area at least 40×40; tooltip required.

### Command palette

The command palette is the signature surface. It should feel like the app's native
launcher:

- Centered, 720px max width, 16px radius, `surface` background, hairline border.
- Search input at the top, 44px, no separate label.
- Results are 40px rows with 6px radius; active row is `surface-card`.
- Right-aligned keycaps for shortcuts.
- Keyboard: ArrowUp/Down, Enter, Escape, Ctrl/Cmd+K.
- Empty state: one line of muted text, no illustration.
- The palette is the only place where Acrylic may be used when Material is enabled.

### Poster cards

- 2:3 aspect ratio; art edge-to-edge; 10px radius; 1px hairline.
- Title below the art, 13–14px medium, single line, ellipsis.
- Favorite star appears on hover/focus only; pinned favorites sort first.
- Running state: 1px green hairline plus a small timer badge; do not use a thick glow.
- Hover: art scales to 1.02, overlay fades in over `normal`; no card lift or shadow.
- Missing art: surface-card background, centered monogram, no broken-image icon.

### Quest dock

- Appears only on the game detail screen while a game is running.
- Shows phase (`Playing`, `Syncing`, `Complete`), remaining time, and a 3px progress
  bar.
- Timer uses tabular numerals; phase changes are announced with `aria-live="polite"`.
- Do not update the accessible label every second; announce only phase changes.

### Red stripe

- A 3px diagonal-stripe gradient from `stripe-start` to `stripe-end`.
- Use once per screen maximum. Recommended placements: the very top edge of the empty
  state, or the top edge of the About/onboarding surface.
- Never place body text directly on the stripe.
- Do not use the stripe as a background for the whole hero; it is a signature accent,
  not a theme.

## Screen structure

### Library

```text
┌ title bar ───────────────────────────────────────────────┐
│ Dummies          [1 active]          [—] [□] [×]         │
├ command bar ─────────────────────────────────────────────┤
│ [Search…]   [All] [Favorites] [Active]      [+ Add game] │
├ poster wall ─────────────────────────────────────────────┤
│ [poster] [poster] [poster] [poster] [poster] [poster]    │
│ [poster] [poster] [poster] [poster] [poster] [poster]    │
└──────────────────────────────────────────────────────────┘
```

- No sidebar. No permanent console.
- The command bar is the only chrome above the art.
- Empty state: one short headline, one sentence, primary Add game button, and a
  three-step explanation. Use the red stripe once at the top edge.
- The activity drawer is collapsed by default; a small status chip in the title bar
  indicates running games.

### Game detail

```text
┌ title bar ───────────────────────────────────────────────┐
│ ← Library    Game name                    [—] [□] [×]   │
├ hero ────────────────────────────────────────────────────┤
│ blurred cover backdrop + near-black gradient             │
│  ┌poster┐   Game title (display-lg)                      │
│  │      │   App ID · Executable · Steam status           │
│  └──────┘   [Launch game]  [More]                         │
│                                      ┌ quest dock ─────┐ │
│                                      │ Playing  12:34  │ │
│                                      └─────────────────┘ │
├ details disclosure ──────────────────────────────────────┤
│ App ID / executable / Steam integration / shortcut       │
└──────────────────────────────────────────────────────────┘
```

- The backdrop is a blurred, darkened cover with a near-black gradient. No bright
  color behind text.
- The primary CTA is the only white (dark mode) or black (light mode) element.
- No console area. Activity opens as a drawer.
- If Steam is required and not configured, show a compact inline warning with a
  secondary action.

### Add game

- A full command palette, not a table modal.
- Search Discord's detectable catalog in Rust; page and virtualize results.
- Each row: name, executable, App ID, and a small `Add` affordance.
- Selecting a row shows a compact preview on the right at ≥1000px; below that, the
  preview becomes an expandable row.
- New-detection warning uses a small dialog, not a separate page.

### Settings

- A dedicated surface, not a modal.
- Left navigation (General, Appearance, Quest automation, Integrations, Updates &
  About) with 6px active radius.
- Rows are separated by hairlines; no nested cards.
- Appearance includes theme, material (Solid/Mica/Acrylic where available), accent
  (System/Custom), motion, language, card size, and view mode.
- Updates uses a secondary button and a compact status line; release notes open in a
  dialog with sanitized Markdown.

## Motion

- Hover/focus: `fast` 120ms.
- Dialog/palette: `normal` 180ms, opacity + scale 0.98 → 1.
- Page transition: `page` 300ms, opacity + 8px translate.
- Poster → detail: shared-element transition when supported (View Transitions API);
  fallback to fade.
- Progress: CSS width/transform transitions; no spring/bounce.
- Avoid animating layout properties. Animate `opacity` and `transform`.
- `prefers-reduced-motion: reduce`: no transform/shared-element motion; keep 80ms
  opacity changes.

## Iconography

- Fluent System Icons, 20px regular and 16px compact; 1.5px optical stroke.
- Monochrome by default; color only for semantic status.
- Steam icon is a custom monochrome glyph (trademark of Valve); do not recolor it as a
  brand accent.
- Window controls use native-like SVG glyphs, not Fluent icons.
- Tray icon: simplified monochrome mark that remains legible at 16×16.

## Copy and localization

- UI languages: English and Spanish. Default: system language; fallback: English.
- Sentence case. No exclamation marks. No "Discord" in the application title or main
  navigation. Use "quest" for the feature domain.
- Buttons are verbs: `Launch`, `Stop`, `Add game`, `Save changes`, `Check for updates`.
- Spanish examples: `Iniciar`, `Detener`, `Añadir juego`, `Guardar cambios`,
  `Buscar actualizaciones`.
- All strings live in locale files. No hardcoded user-facing text in components.
- Use `Intl.DateTimeFormat` / `Intl.NumberFormat` for dates and numbers.
- Error messages state the cause and the next action; do not expose stack traces.
- Logs may use technical English, but user-facing log entries use locale keys.

## Accessibility

- Visible focus ring: 2px `focus`, 2px offset; never remove outline without replacement.
- Keyboard: Tab order follows visual order; grid supports arrow-key navigation;
  Enter opens; Space toggles; context menu key opens the menu.
- Dialog focus trap and restore; Escape closes the topmost surface.
- Contrast: text meets WCAG AA; do not put body text directly on Mica/acrylic.
- High contrast: force Solid, use system colors, keep hairlines visible.
- Do not rely on color alone for running/error states; pair with text or icon.
- Timer changes are not announced every second.

## Anti-patterns

- Dashboard KPI cards, sidebar navigation, permanent terminal panel.
- White CTA plus a second colored primary button.
- Drop shadows on cards, menus, or dialogs.
- Acrylic on the main content area.
- Saturated red/yellow/blue on chrome outside the single stripe and semantic states.
- 16px radius on every element; reserve it for the command palette and hero surfaces.
- Generic shadcn dark theme with a Discord blurple accent.
- Over-animated hover states, bouncing dialogs, or parallax on the poster wall.
- Using "Discord" in the window title, installer title, tray tooltip, or main nav.

## Open items

- Confirm whether Light mode ships in 1.0 or follows in 1.1. Current plan: ship both,
  dark canonical, system default.
- Confirm whether Mica should become the default on Windows 11. Current plan: Solid by
  default; Mica optional in Appearance (title bar only), Solid on Windows 10.
- App icon and tray icon need a dedicated monochrome pass before release.
