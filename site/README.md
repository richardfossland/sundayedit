# SundayEdit landing site (Phase 9.3)

A dependency-free static marketing site for SundayEdit. No build step, no
framework — just `index.html`, `styles.css` and `main.js`, so it serves from any
static host and is trivial to preview and verify.

## Preview locally

```bash
cd site
python3 -m http.server 8080
# open http://localhost:8080
```

Any static server works (`npx serve`, `caddy file-server`, etc.).

## Design

- **Aesthetic:** broadcast-grade precision — dark, cinematic, editorial.
- **Brand:** the app's confident teal (OKLCH hue 195) mirrored from
  `src/styles/tokens.css`, plus the product's confidence colour scale
  (amber → orange-red) as the signature motif.
- **Type:** Bricolage Grotesque (display), Inter (body), JetBrains Mono
  (timecode/data) — the same families the desktop app uses.
- **Signature element:** the hero's interactive _Review threshold_ slider, which
  highlights only the words scoring below the threshold — the killer feature
  (confidence highlighting) made tangible before download.
- **Accessibility:** semantic landmarks, keyboard-focusable controls,
  colour tints paired with underlines, `prefers-reduced-motion` honoured, and
  full readability with JavaScript disabled.

## Content honesty

Claims track the product's own positioning in `CLAUDE.md` (the 92%/8% framing,
local-first privacy, open export formats, "a third of the price"). No fabricated
prices, testimonials, or usage numbers. Download buttons point at the GitHub
releases page.

## TODO before going live

- Replace the OG/preview with a real share image (`og:image`).
- Wire the macOS/Windows buttons to direct asset URLs once a release is
  published (currently the releases page).
- Add a real domain + analytics if desired.
