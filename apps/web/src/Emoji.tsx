// A task's, project's or block's emoji, drawn beside its name as other planners draw an icon, and
// never said: a row's name is the core's line, which never holds the emoji, so the mark is hidden
// from a screen reader and from braille alike. The core sends only a whole emoji (`emoji::shown`);
// one is chosen with the system's own picker in the forms' Emoji line, never from a set kept here.

export function EmojiMark(props: { emoji: string | null | undefined }) {
  const emoji = props.emoji;
  if (!emoji) return null;
  return (
    <span className="emoji" aria-hidden="true">
      {emoji}
    </span>
  );
}
