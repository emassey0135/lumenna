package io.github.emassey0135.lumenna

// What a person chooses among, for the phone's prompts and the watch's.

/** Something offered for choosing: what identifies it, and how it reads. */
data class Choice(val key: String, val title: String, val detail: String = "")

/** The open tasks, less [excluding], to choose from. */
fun taskChoices(core: Core, excluding: Set<String> = emptySet()): List<Choice> =
    core.attempt { core.lumenna.listTasks("") }?.rows.orEmpty()
        .filter { it.id !in excluding }
        .map { Choice(it.id, it.title, RowSpeech.details(it).orEmpty()) }

/** The work blocks a task could go in: which ones is the core's (`workBlocks`). */
fun blockChoices(core: Core): List<Pair<Choice, String>> =
    core.attempt { core.lumenna.workBlocks(null, null) }?.blocks.orEmpty().map { block ->
        Choice(
            block.id,
            "${Clock.spokenDay(block.date)}, ${Clock.time(block.start)}, ${block.title}",
            "${Clock.time(block.start)} to ${Clock.time(block.end)}",
        ) to block.date
    }
