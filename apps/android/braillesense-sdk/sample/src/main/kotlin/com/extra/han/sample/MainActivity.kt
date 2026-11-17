package com.extra.han.sample

import android.app.Activity
import android.os.Bundle
import android.view.KeyEvent
import dev.emassey0135.braillesense.session.BrailleSenseSession
import dev.emassey0135.braillesense.ui.Action
import dev.emassey0135.braillesense.ui.BrailleSenseUi
import dev.emassey0135.braillesense.ui.Item

/**
 * A list of fruit: Enter on one says it, Enter-I on one offers Delete, and Space-M offers Add, which
 * asks for a name and a size. Escape on the list, or Exit, leaves.
 */
class MainActivity : Activity() {
    private val session by lazy { BrailleSenseSession(this, BuildConfig.VERSION_NAME) }
    private lateinit var ui: BrailleSenseUi
    private val fruit = mutableListOf("Apple", "Banana", "Cherry")

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        ui = BrailleSenseUi.on(session)
        ui.onEmpty = ::finish
        session.created()
        ui.list(
            "Fruit",
            items = {
                fruit.map { name ->
                    Item(name, name, open = { ui.message(name) }, actions = listOf(Action("Delete", 'd', asks = true) { remove(name) }))
                }
            },
            actions = { listOf(Action("Add", 'a', asks = true) { add() }) },
        )
    }

    private fun add() = ui.launch {
        val name = ui.askLine("Name") ?: return@launch
        val size = ui.choose("Size", listOf("Small", "Large")) ?: return@launch
        val added = "$name, ${if (size == 0) "small" else "large"}"
        fruit += added
        // Moved to what was made, and said.
        ui.land(added)
    }

    private fun remove(name: String) = ui.launch {
        if (ui.confirm("Delete $name?")) {
            fruit.remove(name)
            ui.announce("Deleted")
            ui.refresh()
        }
    }

    override fun onResume() {
        super.onResume()
        session.resumed()
    }

    override fun onPause() {
        // First: whatever else leaving does, the keys and the display are the next app's.
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

    override fun dispatchKeyEvent(event: KeyEvent): Boolean = session.keyEvent(event) || super.dispatchKeyEvent(event)
}
