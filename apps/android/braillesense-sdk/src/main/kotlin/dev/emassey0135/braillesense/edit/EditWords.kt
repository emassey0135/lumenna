package dev.emassey0135.braillesense.edit

/**
 * What the BrailleSense's edit box says as it is edited, in the device's own words (its
 * `COMMON_EDIT_*` and `COMMON_MSG_*` strings), so that an app's boxes sound like the device's.
 * [ENGLISH] is the device's English, in sentence case, which costs no capital signs on the cells.
 */
data class EditWords(
    /** A new line typed with Enter. */
    val enter: String,
    /** Backspace over a new line, back onto the line before. */
    val previousLine: String,
    /** A line with nothing on it, moved to or read. */
    val blank: String,
    /** A space, typed or taken away. */
    val space: String,
    /** A selection started at the cursor. */
    val startSelection: String,
    /** Everything selected. */
    val selectAll: String,
    /** A selection left with Escape. */
    val selectionCanceled: String,
    /** The selection copied to the clipboard. */
    val copied: String,
    /** The selection cut to the clipboard. */
    val cut: String,
    /** The clipboard pasted at the cursor. */
    val pasted: String,
    /** The selection added to what the clipboard holds. */
    val appended: String,
    /** The clipboard emptied. */
    val clipboardCleared: String,
    /** A selection, a line or a paragraph taken away. */
    val deleted: String,
    /** Copy, cut or read with nothing selected. */
    val nothingSelected: String,
    /** Paste with nothing on the clipboard. */
    val clipboardEmpty: String,
    /** Backspace-2-5 at the end of a line of a multi-line box, joining the next one on. */
    val newParagraph: String,
    /** Asked before Backspace or Space-D takes a selection. */
    val deleteSelected: String,
) {
    companion object {
        val ENGLISH = EditWords(
            enter = "Enter",
            previousLine = "previous line",
            blank = "blank",
            space = "Space",
            startSelection = "Start selection",
            selectAll = "Select all",
            selectionCanceled = "Selection canceled.",
            copied = "Copy complete",
            cut = "Cut",
            pasted = "Paste",
            appended = "Add to clipboard",
            clipboardCleared = "Clear clipboard",
            deleted = "Delete complete.",
            nothingSelected = "Block is not set.",
            // The device beeps here; a word, so that a press is never left unanswered.
            clipboardEmpty = "The clipboard is empty.",
            newParagraph = "new paragraph",
            deleteSelected = "Delete selected text?",
        )
    }
}
