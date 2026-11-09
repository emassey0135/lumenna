package com.extra.han.lumenna.ui

import com.extra.han.lumenna.device.BrailleSettings
import dev.emassey0135.lumenna.app.ControlRole
import dev.emassey0135.lumenna.app.DeviceLine
import dev.emassey0135.lumenna.app.DeviceReading
import dev.emassey0135.lumenna.app.deviceReading

/** How the core reads a line in the person's settings; the JVM tests give their own. */
fun interface Readings {
    fun read(line: DeviceLine): DeviceReading
}

class CoreReadings(private val settings: BrailleSettings) : Readings {
    override fun read(line: DeviceLine): DeviceReading = deviceReading(line, settings.lines, settings.signs)
}

/** A line to read: the text said and shown alike unless [text] says otherwise. */
fun line(
    role: ControlRole,
    speech: String,
    text: String = speech,
    label: String = "",
    detail: String = "",
    letter: Char? = null,
    hotKey: String? = null,
    asks: Boolean = false,
    position: Int = 0,
    count: Int = 0,
) = DeviceLine(role, label, speech, text, detail, letter?.toString(), hotKey, asks, position.toUInt(), count.toUInt())

/** A reading as a frame shows it, an edit box's own [cells] in place of its text. */
fun DeviceReading.rendered(cells: ByteArray? = null, cellCursor: Int = -1) =
    Rendered(speech, body, cells = cells, cellCursor = cellCursor, before = before, after = after)
