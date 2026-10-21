// A row's line on a braille display, apart from what is said (`Line::braille`: states short,
// lengths as hours and minutes): `aria-braillelabel`, which JAWS and NVDA read. Set on the row
// itself, which is what has the name, from inside it: React Aria builds the row and passes on
// only the attributes it knows. Left off where braille and speech are the same.

/** A ref for an element inside a row, giving that row `braille` where it is not `text`. */
export function brailleOnRow(text: string, braille: string) {
  return (inside: HTMLElement | null) => {
    const row = inside?.closest('[role="row"], [role="treeitem"]');
    if (!row) return;
    if (braille && braille !== text) row.setAttribute("aria-braillelabel", braille);
    else row.removeAttribute("aria-braillelabel");
  };
}
