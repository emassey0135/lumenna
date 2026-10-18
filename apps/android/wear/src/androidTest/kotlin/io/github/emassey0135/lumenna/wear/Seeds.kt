package io.github.emassey0135.lumenna.wear

import io.github.emassey0135.lumenna.app.NewBlock
import io.github.emassey0135.lumenna.app.ReplanPins

// What the tests seed with, the rest left as a new block's form leaves it: BoltFFI's records
// have no defaults of their own.

/** A block as the tests add one. */
fun newBlock(title: String, at: String, minutes: UInt, date: String?, kind: String, repeat: String?) =
    NewBlock(title, at, minutes, date, kind, repeat, null, null, null, null, null, null, null, null)

/** No pins: the day proposed as it stands. */
val noPins = ReplanPins(emptyList(), emptyList(), emptyList(), emptyList())
