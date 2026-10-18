package io.github.emassey0135.lumenna

/**
 * Something to do to a row: its name, as TalkBack and the long-press menu both say it. [kind]
 * is the core's, for a key to find the row's action of that kind.
 */
data class RowAction(
    val name: String,
    val kind: io.github.emassey0135.lumenna.app.ActionKind? = null,
    val destructive: Boolean = false,
    /** What the core's action is done to, so a key with nothing to do here can say why. */
    val subject: io.github.emassey0135.lumenna.app.Subject? = null,
    /** Whether a row shows it by itself, before the rest (`Action.primary`). */
    val primary: Boolean = false,
    val run: () -> Unit,
)

/** The first of [this] of one of [kinds]: a key's action on the row in hand. */
fun List<RowAction>.ofKind(vararg kinds: io.github.emassey0135.lumenna.app.ActionKind): RowAction? =
    firstOrNull { it.kind != null && it.kind in kinds }
