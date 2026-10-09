package io.github.emassey0135.lumenna

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import io.github.emassey0135.lumenna.core.Direction
import io.github.emassey0135.lumenna.core.parseWeight

// What can be done to a project, a label or a saved filter, wherever one is listed: Browse's
// screens, and the sidebar of a wide window. Once, so the two never offer different things.

/** A project's: rename, reorder, nest, weigh, archive, delete. `others` are the projects it can go under. */
fun projectActions(core: Core, prompt: Prompter, name: String, archived: Boolean, others: () -> List<String>): List<RowAction> =
    listOf(
        RowAction("Rename") {
            prompt.show {
                AskText("Rename $name", "Name", "Rename", initial = name, dismiss = prompt::close) { renamed ->
                    prompt.close()
                    core.change { it.renameProject(name, renamed.trim()) }
                }
            }
        },
        RowAction("Move Up") { core.change { it.reorderProject(name, Direction.UP) } },
        RowAction("Move Down") { core.change { it.reorderProject(name, Direction.DOWN) } },
        RowAction("Move Under") {
            prompt.show {
                val choices = listOf(Choice("", "Top Level")) + others().filter { it != name }.map { Choice(it, it) }
                Choose("Move $name under", choices, "", prompt::close) { parent ->
                    prompt.close()
                    core.change { it.moveProject(name, parent.key.ifEmpty { null }) }
                }
            }
        },
        RowAction("Add Project Inside") { addProject(core, prompt, inside = name) },
        RowAction("Weight") {
            prompt.show {
                AskText(
                    "Weight of $name", "Weight", "Set", example = "1.0",
                    hint = "How much this whole area matters now, roughly 0.5 to 2. Type inherit to take the parent's again.",
                    dismiss = prompt::close,
                ) { text ->
                    // A typo must not quietly become "inherit": the core reads it, and a
                    // refusal is said and leaves the question open with what was typed.
                    val weight = core.attempt { parseWeight(text) } ?: return@AskText
                    prompt.close()
                    core.change { it.weighProject(name, weight) }
                }
            }
        },
        RowAction(if (archived) "Unarchive" else "Archive") { core.change { it.archiveProject(name) } },
        RowAction("Delete") {
            prompt.show {
                AlertDialog(
                    onDismissRequest = prompt::close,
                    title = { Text("Delete $name?") },
                    text = { Text("Its tasks can go to the trash with it, or move to the Inbox.") },
                    confirmButton = {
                        Column {
                            TextButton(modifier = Target, onClick = {
                                prompt.close()
                                core.change { it.deleteProject(name, false) }
                            }) { Text("Delete and Trash Its Tasks") }
                            TextButton(modifier = Target, onClick = {
                                prompt.close()
                                core.change { it.deleteProject(name, true) }
                            }) { Text("Delete and Keep Its Tasks") }
                            TextButton(modifier = Target, onClick = prompt::close) { Text("Cancel") }
                        }
                    },
                )
            }
        },
    )

/** Asks for a new project's name, at the top level or `inside` another. */
fun addProject(core: Core, prompt: Prompter, inside: String? = null) {
    prompt.show {
        AskText(inside?.let { "New Project in $it" } ?: "New Project", "Name", "Add", dismiss = prompt::close) { name ->
            prompt.close()
            core.change { it.addProject(name.trim(), inside) }
        }
    }
}

/** A label's: rename, reorder, merge, colour, delete. `others` are the labels it can merge into. */
fun labelActions(core: Core, prompt: Prompter, name: String, others: () -> List<String>): List<RowAction> = listOf(
    RowAction("Rename") {
        prompt.show {
            AskText("Rename $name", "Name", "Rename", initial = name, dismiss = prompt::close) { renamed ->
                prompt.close()
                core.change { it.renameLabel(name, renamed.trim()) }
            }
        }
    },
    RowAction("Move Up") { core.change { it.reorderLabel(name, Direction.UP) } },
    RowAction("Move Down") { core.change { it.reorderLabel(name, Direction.DOWN) } },
    RowAction("Merge Into") {
        prompt.show {
            Choose("Merge $name into", others().filter { it != name }.map { Choice(it, it) }, "There are no other labels.", prompt::close) { into ->
                prompt.close()
                core.change { it.mergeLabels(name, into.key) }
            }
        }
    },
    RowAction("Colour") {
        prompt.show {
            AskText(
                "Colour of $name", "Colour", "Set", example = "teal",
                hint = "A colour name, such as teal or orange. Empty for none.", dismiss = prompt::close,
            ) { colour ->
                prompt.close()
                core.change { it.recolourLabel(name, colour.trim().ifEmpty { null }) }
            }
        }
    },
    RowAction("Delete") {
        prompt.show {
            Confirm("Delete $name?", "Tasks wearing it stay; they just stop showing it.", "Delete", prompt::close) {
                prompt.close()
                core.change { it.deleteLabel(name) }
            }
        }
    },
)

/** Asks for a new label's name. */
fun addLabel(core: Core, prompt: Prompter) {
    prompt.show {
        AskText("New Label", "Name", "Add", dismiss = prompt::close) { name ->
            prompt.close()
            core.change { it.addLabel(name.trim()) }
        }
    }
}

/** A saved filter's: rename, change its query, reorder, delete. */
fun filterActions(core: Core, prompt: Prompter, name: String, query: String): List<RowAction> = listOf(
    RowAction("Rename") {
        prompt.show {
            AskText("Rename $name", "Name", "Rename", initial = name, dismiss = prompt::close) { renamed ->
                prompt.close()
                core.change { it.editFilter(name, renamed.trim(), null) }
            }
        }
    },
    RowAction("Change Query") {
        prompt.show {
            AskText("Query for $name", "Query", "Save", initial = query, dismiss = prompt::close) { changed ->
                prompt.close()
                core.change { it.editFilter(name, null, changed.trim()) }
            }
        }
    },
    RowAction("Move Up") { core.change { it.reorderFilter(name, Direction.UP) } },
    RowAction("Move Down") { core.change { it.reorderFilter(name, Direction.DOWN) } },
    RowAction("Delete") {
        prompt.show {
            Confirm("Delete $name?", "The tasks it shows are not touched.", "Delete", prompt::close) {
                prompt.close()
                core.change { it.deleteFilter(name) }
            }
        }
    },
)

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
