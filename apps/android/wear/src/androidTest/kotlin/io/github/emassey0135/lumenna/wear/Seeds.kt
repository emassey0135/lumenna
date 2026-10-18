package io.github.emassey0135.lumenna.wear

import androidx.compose.ui.test.IdlingResource
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.app.NewBlock
import io.github.emassey0135.lumenna.app.ReplanPins

// What the tests seed with and wait on.

/** A block as the tests add one, the rest as a new block's form leaves it: BoltFFI's records have no defaults. */
fun newBlock(title: String, at: String, minutes: UInt, date: String?, kind: String, repeat: String?) =
    NewBlock(title, at, minutes, date, kind, repeat, null, null, null, null, null, null, null, null)

/** No pins: the day proposed as it stands. */
val noPins = ReplanPins(emptyList(), emptyList(), emptyList(), emptyList())

/**
 * The app as Compose's test rule waits on it: busy while the store works or a turn is on its
 * way to the screen, so no test looks at the screen between what it did and the app's answer.
 */
fun stillApp(core: Core) = object : IdlingResource {
    override val isIdleNow: Boolean get() = core.still
}
