package io.github.emassey0135.lumenna

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import io.github.emassey0135.lumenna.core.Direction
import io.github.emassey0135.lumenna.core.parseWeight

// What can be done to a project, a label or a saved filter, wherever one is listed: Browse's
// screens, and the sidebar of a wide window. Once, so the two never offer different things.

/** A project's, as the watch offers them too (`PlaceAction.ofProject`). `others` are the
 *  projects it can go under. */
fun projectActions(core: Core, prompt: Prompter, name: String, archived: Boolean, others: () -> List<String>): List<RowAction> =
    PlaceAction.ofProject(archived).mapNotNull { action ->
        val run: (() -> Unit)? = when (action) {
            PlaceAction.RENAME -> { {
                prompt.show {
                    AskText("Rename $name", "Name", "Rename", initial = name, dismiss = prompt::close) { renamed ->
                        prompt.close()
                        core.change { it.renameProject(name, renamed.trim()) }
                    }
                }
            } }
            PlaceAction.MOVE_UP -> { { core.change { it.reorderProject(name, Direction.UP) } } }
            PlaceAction.MOVE_DOWN -> { { core.change { it.reorderProject(name, Direction.DOWN) } } }
            PlaceAction.MOVE_UNDER -> { {
                prompt.show {
                    val choices = listOf(Choice("", "Top Level")) + others().filter { it != name }.map { Choice(it, it) }
                    Choose("Move $name under", choices, "", prompt::close) { parent ->
                        prompt.close()
                        core.change { it.moveProject(name, parent.key.ifEmpty { null }) }
                    }
                }
            } }
            PlaceAction.ADD_PROJECT_INSIDE -> { { addProject(core, prompt, inside = name) } }
            PlaceAction.WEIGHT -> { {
                prompt.show {
                    AskText(
                        "Weight of $name", "Weight", "Set", example = "1.0",
                        hint = PlaceAction.WEIGHT_HELP,
                        dismiss = prompt::close,
                    ) { text ->
                        // A typo must not quietly become "inherit": the core reads it, and a
                        // refusal is said and leaves the question open with what was typed.
                        val weight = core.attempt { parseWeight(text) } ?: return@AskText
                        prompt.close()
                        core.change { it.weighProject(name, weight) }
                    }
                }
            } }
            PlaceAction.ARCHIVE, PlaceAction.UNARCHIVE -> { { core.change { it.archiveProject(name) } } }
            PlaceAction.DELETE -> { {
                prompt.show {
                    AlertDialog(
                        onDismissRequest = prompt::close,
                        title = { Text("Delete $name?") },
                        text = { Text(PlaceAction.DELETING_PROJECT) },
                        confirmButton = {
                            Column {
                                TextButton(modifier = Target, onClick = {
                                    prompt.close()
                                    core.change { it.deleteProject(name, false) }
                                }) { Text(PlaceAction.DELETE_AND_TRASH) }
                                TextButton(modifier = Target, onClick = {
                                    prompt.close()
                                    core.change { it.deleteProject(name, true) }
                                }) { Text(PlaceAction.DELETE_AND_KEEP) }
                                TextButton(modifier = Target, onClick = prompt::close) { Text("Cancel") }
                            }
                        },
                    )
                }
            } }
            else -> null
        }
        run?.let { RowAction(action.title, it) }
    }

/** Asks for a new project's name, at the top level or `inside` another. */
fun addProject(core: Core, prompt: Prompter, inside: String? = null) {
    prompt.show {
        AskText(inside?.let { "New Project in $it" } ?: "New Project", "Name", "Add", dismiss = prompt::close) { name ->
            prompt.close()
            core.change { it.addProject(name.trim(), inside) }
        }
    }
}

/** A label's, as the watch offers them too (`PlaceAction.ofLabel`). `others` are the
 *  labels it can merge into. */
fun labelActions(core: Core, prompt: Prompter, name: String, others: () -> List<String>): List<RowAction> =
    PlaceAction.ofLabel.mapNotNull { action ->
        val run: (() -> Unit)? = when (action) {
            PlaceAction.RENAME -> { {
                prompt.show {
                    AskText("Rename $name", "Name", "Rename", initial = name, dismiss = prompt::close) { renamed ->
                        prompt.close()
                        core.change { it.renameLabel(name, renamed.trim()) }
                    }
                }
            } }
            PlaceAction.MOVE_UP -> { { core.change { it.reorderLabel(name, Direction.UP) } } }
            PlaceAction.MOVE_DOWN -> { { core.change { it.reorderLabel(name, Direction.DOWN) } } }
            PlaceAction.MERGE_INTO -> { {
                prompt.show {
                    Choose("Merge $name into", others().filter { it != name }.map { Choice(it, it) }, "There are no other labels.", prompt::close) { into ->
                        prompt.close()
                        core.change { it.mergeLabels(name, into.key) }
                    }
                }
            } }
            PlaceAction.COLOUR -> { {
                prompt.show {
                    AskText(
                        "Colour of $name", "Colour", "Set", example = "teal",
                        hint = PlaceAction.COLOUR_HELP, dismiss = prompt::close,
                    ) { colour ->
                        prompt.close()
                        core.change { it.recolourLabel(name, colour.trim().ifEmpty { null }) }
                    }
                }
            } }
            PlaceAction.DELETE -> { {
                prompt.show {
                    Confirm("Delete $name?", PlaceAction.DELETING_LABEL, "Delete", prompt::close) {
                        prompt.close()
                        core.change { it.deleteLabel(name) }
                    }
                }
            } }
            else -> null
        }
        run?.let { RowAction(action.title, it) }
    }

/** Asks for a new label's name. */
fun addLabel(core: Core, prompt: Prompter) {
    prompt.show {
        AskText("New Label", "Name", "Add", dismiss = prompt::close) { name ->
            prompt.close()
            core.change { it.addLabel(name.trim()) }
        }
    }
}

/** A saved filter's, as the watch offers them too (`PlaceAction.ofFilter`). */
fun filterActions(core: Core, prompt: Prompter, name: String, query: String): List<RowAction> =
    PlaceAction.ofFilter.mapNotNull { action ->
        val run: (() -> Unit)? = when (action) {
            PlaceAction.RENAME -> { {
                prompt.show {
                    AskText("Rename $name", "Name", "Rename", initial = name, dismiss = prompt::close) { renamed ->
                        prompt.close()
                        core.change { it.editFilter(name, renamed.trim(), null) }
                    }
                }
            } }
            PlaceAction.CHANGE_QUERY -> { {
                prompt.show {
                    AskText("Query for $name", "Query", "Save", initial = query, dismiss = prompt::close) { changed ->
                        prompt.close()
                        core.change { it.editFilter(name, null, changed.trim()) }
                    }
                }
            } }
            PlaceAction.MOVE_UP -> { { core.change { it.reorderFilter(name, Direction.UP) } } }
            PlaceAction.MOVE_DOWN -> { { core.change { it.reorderFilter(name, Direction.DOWN) } } }
            PlaceAction.DELETE -> { {
                prompt.show {
                    Confirm("Delete $name?", PlaceAction.DELETING_FILTER, "Delete", prompt::close) {
                        prompt.close()
                        core.change { it.deleteFilter(name) }
                    }
                }
            } }
            else -> null
        }
        run?.let { RowAction(action.title, it) }
    }

/** Asks for a new saved filter's name, then its query. */
fun addFilter(core: Core, prompt: Prompter) {
    prompt.show {
        AskText("New Filter", "Name", "Next", dismiss = prompt::close) { name ->
            prompt.show {
                AskText("Query for $name", "Query", "Save", example = "#Work & overdue", dismiss = prompt::close) { query ->
                    prompt.close()
                    core.change { it.addFilter(name.trim(), query.trim()) }
                }
            }
        }
    }
}
