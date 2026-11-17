package dev.emassey0135.orbit.sample

import android.app.Activity
import android.os.Bundle
import dev.emassey0135.orbit.session.OrbitSession
import dev.emassey0135.orbit.ui.Action
import dev.emassey0135.orbit.ui.Field
import dev.emassey0135.orbit.ui.Item
import dev.emassey0135.orbit.ui.OrbitUi

/**
 * A list of notes on an Orbit Speak or Player: Select edits one, Space with D (or the menu key's
 * Delete) deletes it, and the menu key offers New note and Sort. Everything said is Orbit's own
 * screen reader's, in the person's voice, from what the stack tells it.
 */
class MainActivity : Activity() {
    private class Note(val id: Int, var text: String, var pinned: Boolean = false)

    private lateinit var session: OrbitSession
    private lateinit var ui: OrbitUi
    private val notes = mutableListOf(Note(1, "Buy milk"), Note(2, "Call the bank"))
    private var nextId = 3
    private var newestFirst = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        session = OrbitSession(this, "Notes", debugKeys = "dev.emassey0135.orbit.sample.DEBUG_KEY")
        ui = OrbitUi.on(session)
        ui.onEmpty = ::finish
        ui.list(
            "Notes", ::rows, empty = "No notes",
            actions = { listOf(Action("New note", 'n') { newNote() }, Action("Sort", 's', asks = true) { sort() }) },
        )
        session.created()
    }

    override fun onResume() {
        super.onResume()
        session.resumed()
    }

    override fun onPause() {
        // First thing: whatever else leaving does, the keypad is the next app's.
        session.paused()
        super.onPause()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        session.windowFocus(hasFocus)
    }

    override fun onDestroy() {
        session.destroyed()
        super.onDestroy()
    }

    private fun rows(): List<Item> = shown().map { note ->
        Item(
            key = note.id.toString(),
            text = note.text,
            open = { edit(note) },
            actions = listOf(
                Action(if (note.pinned) "Unpin" else "Pin", 'p') { pin(note) },
                Action("Delete", 'd') { delete(note) },
            ),
            detail = if (note.pinned) "pinned" else "",
            delete = { delete(note) },
        )
    }

    private fun shown(): List<Note> {
        val ordered = if (newestFirst) notes.sortedByDescending { it.id } else notes
        return ordered.sortedByDescending { it.pinned }
    }

    private fun newNote() = ui.launch {
        val text = ui.askLine("New note")?.takeIf { it.isNotBlank() } ?: return@launch
        val note = Note(nextId++, text)
        notes += note
        // The app moved what is in hand: to the new note, which is told.
        ui.land(note.id.toString())
    }

    private fun edit(note: Note) = ui.launch {
        val text = ui.askLine("Note", initial = note.text)?.takeIf { it.isNotBlank() } ?: return@launch
        note.text = text
        ui.refresh()
    }

    private fun pin(note: Note) {
        note.pinned = !note.pinned
        ui.land(note.id.toString())
    }

    private fun delete(note: Note) = ui.launch {
        if (!ui.confirm("Delete ${note.text}?")) return@launch
        notes.remove(note)
        ui.refresh()
        ui.announce("Deleted")
    }

    private fun sort() = ui.launch {
        val values = ui.form("Sort", listOf(Field.Check("newest", "Newest first", newestFirst))) ?: return@launch
        newestFirst = values.checked("newest")
        ui.refresh()
    }
}
