# The parsers

Read when working in `crates/parse/`. The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

## Parsing

`parse` reads text; what it produces (`DueSpec`, `filter::Expr`, `State`) lives in `core`,
so evaluation, the readback and the accessibility layer never depend on the parser.

- **Both parsers work over words**, not characters, because what matters is knowing which
  *span* a phrase consumed. They are recursive descent: the date grammar is word-level and
  multi-word (`next friday`, `every mon, wed and fri`), so a character-level combinator
  grammar would need two tokenisations bridged at every `due before:`.
  `ParseError::expected` carries the valid token kinds, and completion is built on it.
- **Quick add cuts the title out of the original input**, so punctuation and spacing
  survive.
- **A week is the person's** (`week-start`): "next week" is the first day of next week, "this
  week" and "end of the week" its last day (`DateSpec::WeekStart`, `WeekEnd`), so every date a
  person types resolves through `resolve_in` with the snapshot's week start; `resolve` alone
  assumes Monday. "In a week" is still seven days.
- **A quick-add line that starts with `block` makes a block** (`lumenna_parse::block`), handed
  to `add_block` as the block form would. Its name takes at least the first word, so "Weekly
  review" is a name, not a repetition. A word that is none of name, time, day or repetition is
  refused, named.
- **Nothing is ever silently swallowed.** An unrecognised or repeated token stays in the
  title with a notice. A property test (`quick_add_accounts_for_every_character`) asserts
  every non-space character is in the title or inside a recognised span.

