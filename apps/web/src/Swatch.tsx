// A colour of the core's palette, drawn beside a name and never in place of it: the name, and
// the colour's word in the row's line, are what a screen reader reads, so the mark is hidden
// from it. Its light or dark value follows the page's colour scheme; under forced colours the
// system's palette rules and the mark is left out (styles.css).

import type { Swatch } from "./core";

export function SwatchMark(props: { swatch: Swatch | null | undefined }) {
  const swatch = props.swatch;
  if (!swatch) return null;
  return <span className="swatch" aria-hidden="true" style={{ background: `light-dark(${swatch.light}, ${swatch.dark})` }} />;
}
