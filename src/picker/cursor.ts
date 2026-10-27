// ABOUTME: Inline-SVG cursor for window picker mode. Chromatic crosshair
// ABOUTME: with accent-orange ticks; rendered at 28px with hotspot at centre.

export const PICKER_CURSOR_SVG = `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48" width="28" height="28">
  <circle cx="24" cy="24" r="5" fill="#0c0c0c" stroke="#e0e0e0" stroke-width="1.5"/>
  <circle cx="22.5" cy="24" r="5" fill="none" stroke="rgba(255,0,60,0.85)" stroke-width="1.5"/>
  <circle cx="25.5" cy="24" r="5" fill="none" stroke="rgba(0,245,255,0.85)" stroke-width="1.5"/>
  <line x1="24" y1="4"  x2="24" y2="17" stroke="#ee6633" stroke-width="2"/>
  <line x1="24" y1="31" x2="24" y2="44" stroke="#ee6633" stroke-width="2"/>
  <line x1="4"  y1="24" x2="17" y2="24" stroke="#ee6633" stroke-width="2"/>
  <line x1="31" y1="24" x2="44" y2="24" stroke="#ee6633" stroke-width="2"/>
</svg>
`.trim();

function encodeSvgForCssUrl(svg: string): string {
  // The smallest reliable encoding for CSS cursor URLs across WebKit: percent-encode '<', '>', '#', and '"'.
  return svg
    .replace(/\n/g, "")
    .replace(/"/g, "'")
    .replace(/</g, "%3C")
    .replace(/>/g, "%3E")
    .replace(/#/g, "%23");
}

export const PICKER_CURSOR_CSS = `url("data:image/svg+xml;utf8,${encodeSvgForCssUrl(PICKER_CURSOR_SVG)}") 14 14, crosshair`;
