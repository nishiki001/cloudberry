# Design (v2: classic desktop player)

Replaces the Y2K CD-player design. Target look: a classic library-first desktop music player in the tradition of Strawberry / Clementine / Amarok. Dense, information-rich, native-feeling widgets, a spectrum analyzer in the toolbar, playlist table as the centerpiece. Not a web-app look: no big cards, no huge paddings, no rounded-everything.

**Reference:** https://www.strawberrymusicplayer.org/ (screenshots on the homepage). Match layout, density and feel. Do NOT copy its name, logo, icons or any artwork; do not commit its screenshots. (R0 tells you how to look at them.)

## Layout (default 1200×760, min 860×540)
```
┌ Menu: Music  Playlist  View  Tools  Help ───────────────────────────────────┐
├──────────────────────┬──────────────────────────────────────────────────────┤
│ ▌Now   │             │ ⏮ ▶ ⏹ ⏭ │ ▁▃▅▇▆▄▂▃▅ analyzer ▅▃ │ 🔊━━━●━━ │ ⤨ ⟳ │
│ ▌Library│ panel for   │ 1:23 ━━━━━━━━━━━━━━━━━━●━━━━━━━━━━━━━━━━━━━━ 4:05 │
│ ▌Search │ the active  ├──────────────────────────────────────────────────────┤
│ ▌Lists  │ sidebar tab │ [Queue] [Liked songs] [Radio: …] [+]                 │
│ ▌Lyrics │ (tree/list) │ ┌──┬─────────────────┬───────────┬──────────┬─────┐  │
│         │             │ │▶ │ Title           │ Artist    │ Album    │ 4:05│  │
│ vertical│             │ │  │ alternating rows, sortable, resizable cols │  │
│ icon+   │             │ │  │ playing row: bold + ▶ icon                 │  │
│ label   ├─────────────┤ │  │                                            │  │
│ tabs    │ ◉ cover     │ │  │                                            │  │
│         │ Title       │ └──┴─────────────────┴───────────┴──────────┴─────┘  │
│         │ Artist      │                                                      │
├──────────────────────┴──────────────────────────────────────────────────────┤
│ 37 tracks · 2:31:10   │   AAC 256 kbps   │   signed in                       │
└─────────────────────────────────────────────────────────────────────────────┘
```
- **Left:** vertical tab strip (icon above small label, ~64 px wide) + panel (resizable splitter, default 280 px). Bottom of the left pane: now-playing widget (cover, title, artist), click → Now tab.
- **Sidebar tabs:**
  - **Now:** large cover (spinning-disc mode optional, see below), title, artist, album, year, then synced lyrics with the current line highlighted.
  - **Library:** expandable tree: Liked songs · Playlists ▸ · Albums ▸ · Artists ▸ · History. Double-click = open as playlist tab; drag into table = append.
  - **Search:** search field, filter chips (Songs, Albums, Artists, Playlists, Videos), result list with small thumbnails. Double-click = add & play, Enter = append.
  - **Lists:** saved/own playlists, "open in new tab".
- **Top right:** transport (prev, play/pause, stop, next) · analyzer (expands to fill) · volume slider · shuffle/repeat toggles. Below: seek slider with elapsed / total time.
- **Center:** playlist tabs (closeable, rename, reorder) + playlist table. This is where the eye lives.
- **Status bar:** track count + total length of current playlist · current stream format/bitrate · sign-in state · transient messages (errors, "added 12 tracks").
- **Menu bar:** Music (Play/Pause, Stop, Next, Prev, Like, Quit) · Playlist (New, Rename, Close, Clear, Shuffle, Save as…) · View (Theme ▸, Analyzer ▸ Block/Bars/Off, Mini player, Show sidebar) · Tools (Settings, Sign in / out, Clear cache) · Help (About, Keyboard shortcuts).
- **Mini player** (View → Mini player, Ctrl+M): cover + title / artist · album + small analyzer + transport + seek, 440×120 (minimum 360×110). Every text elides with "…" and keeps the same right margin as the left one; a button returns to the full window.

## Playlist table
- Columns: ▶ state · # · Title · Artist · Album · Year · Length · ♥. Header right-click toggles columns; drag to resize/reorder; click to sort; persisted in config.
- Row height 22–24 px, alternating row tint, 1px gridless (no vertical lines), selection = accent tint, playing row bold with ▶.
- Multi-select (Ctrl/Shift), drag to reorder, Delete removes, context menu: Play, Play next, Add to queue, Go to artist, Go to album, Like, Remove, Copy link.
- Must stay smooth with 5,000 rows (virtualized list).

## Analyzer ("beat bar")
- **Constant cell size, band count from the width** (F1): Block cells 6×2 px with a 1 px gap, Bars / Mirror bars 4 px wide + 1 px gap, Dots 4 px round dots on a 6 px grid, Wave / Curve 6 px control spacing. Bands = width / pitch, clamped 12–128, log-spaced ~40 Hz–16 kHz again on every resize; the row count follows the height the same way. A wider window shows more columns, never fatter ones.
- Styles: **Block** (default), **Bars**, **Wave** (oscilloscope of the raw samples, peak-picked and smoothed), **Curve** (Catmull-Rom through the band values, filled area below, peak line), **Mirror bars** (grow up and down from the middle line), **Dots / LED**. **Off** hides it (toolbar reflows). Right-click menu, View menu and Settings → Analyzer list them all.
- **Colors** (Settings → Analyzer): Follow theme accent (default), presets Classic (green→yellow→red), Ice, Sunset, Mono, or Custom (1–3 gradient stops via the colour picker); the peak colour is chosen separately (auto = lighter top stop).
- dB scale, fast attack, ~300 ms decay, peaks hold ~400 ms then fall. 30 fps default (setting: 30/60). Animates only while playing; paused → everything falls to zero (silence draws nothing in any style), then **no redraws** (0% CPU).
- Driven by real audio samples (R1/R2). Never fake/random animation.

## Now-playing cover
- Default: flat square cover with a 1 px border.
- Setting "Spinning disc" (keeps the earlier idea): circle-cropped cover with hub, rim and fixed sheen, spinning while playing; reuse the existing Disc component. Calm by default: 1 turn per 8 s; "Disc speed" Slow 12 s / Normal 8 s / Fast 4 s; smooth spin-up and spin-down.

## Visual style
- Native desktop, KDE-Breeze-like, with **light gradients** (not flat, not glossy): buttons, toolbar, table header, tab strips and the selected-row highlight all have a soft top→bottom gradient (top ~4–6% lighter than bottom), a 1 px border slightly darker than the fill, and a faint 1 px inner highlight along the top edge. Pressed = gradient inverted. Panels and the table base stay solid so text reads cleanly.
- System UI font at system size (fallback 10 pt), so Japanese and Cyrillic titles render via fallback.
- Radius 3–4 px on inputs/buttons, 0 on panels. 1 px separators. Subtle hover tints. Focus rings visible.
- Transport buttons: tool buttons (icon only, 28–32 px) with the gradient on hover/press; play/pause slightly larger.
- Gradients are generated from the base colors (lighten/darken), never hand-picked per widget, so they follow custom tones automatically.

## Customization (Settings → Appearance)
- **Tone / colors:** "Use system colors" (default) or custom: pick accent, window background and text colors (the in-app colour picker: HSV square + hue strip, hex input, 8 recent colours and the presets Blue, Teal, Rose, Amber, Violet, Graphite; live preview while dragging, Cancel restores; the same picker is used for the background tint and the analyzer colours). Everything else (selection, alt-row, borders, header, gradients, analyzer colors) is derived from those, separately for light and dark. Live preview while picking.
- **Background image** behind the playlist table:
  - Source: None · Default (a subtle original pattern bundled in-repo) · **Album cover of the playing track** · Custom image (file picker).
  - Options: anchor (full 3×3 grid: top-left … bottom-right, shown in Settings as nine small toggle buttons; old `top` / `center` / `bottom` values keep working), fit (stretch, fill/crop, keep aspect), opacity 0–100%, blur radius 0–30, and a tint (strength slider + colour, default the theme base; colour via the picker).
  - Album-cover mode crossfades (~400 ms) on track change.
  - The table base becomes translucent (alpha from opacity setting) so the image shows through; text contrast must stay ≥ 4.5:1 (darken/lighten overlay automatically if needed).
  - Blur/scale is computed once per image off the UI thread and cached; never per frame.
- All of it persisted in config.toml and applied live, no restart.
- Icons: one open-licensed monochrome SVG set bundled in `assets/icons/` (e.g. Lucide, ISC; or Breeze, LGPL), recolored per theme via `colorize`. License file alongside. One icon size grid (16 px in lists, 20–22 px in toolbar).
- Theme follows OS (`Palette.color-scheme`), override in View → Theme.

## Tokens (`ui/theme.slint`, replace old ones)
| token | Light | Dark |
|---|---|---|
| window | #EFF0F1 | #2A2E32 |
| base (table/panels) | #FCFCFC | #1B1E20 |
| alt-row | #F4F5F6 | #212427 |
| header | #E3E5E7 | #31363B |
| border | #C9CCD0 | #3E444A |
| text / text-dim | #232629 / #6E7378 | #EFF0F1 / #A1A9B1 |
| accent | #3DAEE9 | #3DAEE9 |
| selection | #3DAEE9 @ 25% | #3DAEE9 @ 30% |
| playing-row text | text, bold | text, bold |
| analyzer colours | from the analyzer colour scheme (default: accent dark → accent → accent light) | same |
| row-h / tool-btn | 24 / 30 | same |
| gradient-top / gradient-bottom | lighten(base-of-widget, 5%) / base-of-widget | lighten 4% / base |
| bevel-highlight | white @ 50% | white @ 6% |

These are the defaults for "system colors". Custom tones regenerate the whole table from accent + window + text.

## Keyboard
`Space` play/pause · `Ctrl+→/←` next/prev · `→/←` seek 5 s · `Ctrl+↑/↓` volume · `Ctrl+F` search tab · `Ctrl+L` like · `Ctrl+T` new playlist tab · `Ctrl+W` close tab · `Ctrl+M` mini player · `Ctrl+,` settings · `Alt+1..5` sidebar tabs.

## Visual QA checklist (screenshot both themes, 1x)
Looks like a native desktop music player next to the reference, not a web app · gradients visible but subtle, consistent light direction · screenshots also with a custom tone and with album-cover background · density matches (≥ 20 rows visible at default size) · nothing clipped at min size · CJK + Cyrillic render · icons crisp and same weight · no hardcoded colors · analyzer aligned to toolbar height.

## Reference notes (from the R0 look at the reference screenshots; nothing copied)
- Window ~1430×830: left tab strip ≈ 75 px (icon over label, dark vertical gradient), sidebar panel ≈ 270 px, table fills the rest (≈ 62% of width).
- The reference puts transport + analyzer + volume in a row *below* the table, seek bar + time under that; our DESIGN keeps them on top-right (owner's layout wins).
- Table: header row ≈ 20 px with a faint gradient and column separators, rows ≈ 19 px, alternating tints, selected row = saturated blue fill with white text; sort arrow in header; many narrow right-aligned metadata columns.
- A faded album cover shows through the table as background (≈ 25% opacity, centered, bottom-anchored); text stays readable because rows are semi-opaque.
- Analyzer: dense grid of tiny square blocks (≈ 3 px) in a recessed light-grey well; lit blocks blue→cyan, peak caps fall slowly; idle = empty grid.
- Chrome is flat-ish with light gradients on toolbar/header; icons are colourful there, ours are monochrome. Status bar: track count + total time left, repeat/shuffle toggles, elapsed time, seek, remaining time.
- Sidebar "Context" panel: bold title/artist, cover, then lyrics as plain centered-left text; Collection panel is an album tree with small cover thumbnails.

## Discover (D1)
- Sidebar tab "Discover" (compass icon, Alt+5) listing Home, New releases, Charts, Moods & genres. Choosing one opens the **Discover tab** pinned right after Queue (not closeable); its page replaces the table area.
- Page = header (title, Back for a mood category, Refresh) + shelves stacked vertically. Card shelf: heading + a row of 140 px covers with title and subtitle (elided), scrolling sideways with round arrow buttons at the edges; hover shows a round accent play button on the cover; right-click: Play, Add to queue, Open in new tab, Start radio; click opens the album/playlist/artist in a new tab (songs play). Song shelves (Quick picks…) are compact lists, 4 rows per column, same row style as the table. Charts add a country chip row; moods are a grid of chips tinted from the accent hue; a chip opens that category's playlists as shelves.
- Data from InnerTube browse (home with continuations, new releases, charts, moods) cached on disk 30 min; covers load lazily for visible cards only. Related shelves of the playing track close the Home page; the Now tab has a "More like this" link (radio).
