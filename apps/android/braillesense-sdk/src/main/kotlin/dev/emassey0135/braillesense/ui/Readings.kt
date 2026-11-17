package dev.emassey0135.braillesense.ui

import dev.emassey0135.braillesense.braille.BrailleSettings
import dev.emassey0135.braillesense.output.BrailleDisplay
import dev.emassey0135.braillesense.output.DisplayLine
import dev.emassey0135.braillesense.reading.ControlRole
import dev.emassey0135.braillesense.reading.DeviceLine
import dev.emassey0135.braillesense.reading.DeviceReading
import dev.emassey0135.braillesense.reading.deviceReading

/** How the device reads a line ([deviceReading]): [SettingsReadings] in the person's own settings, or a test's own. */
public fun interface Readings {
    /** How [line] is said and shown. */
    public fun read(line: DeviceLine): DeviceReading
}

/** The device's reading in the person's Global Options, as [BrailleSettings] last read them. */
public class SettingsReadings(private val settings: BrailleSettings) : Readings {
    override fun read(line: DeviceLine): DeviceReading = deviceReading(line, settings.lines, settings.signs)
}

/** A line to read: the text said and shown alike unless [text] says otherwise. */
public fun line(
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
): DeviceLine = DeviceLine(role, label, speech, text, detail, letter?.toString(), hotKey, asks, position, count)

/** A reading as a frame shows it, an edit box's own [cells] in place of its text. */
public fun DeviceReading.rendered(cells: ByteArray? = null, cellCursor: Int = -1): Rendered =
    Rendered(speech, body, cells = cells, cellCursor = cellCursor, before = before, after = after)

/** Puts what a frame shows on the display. */
public fun BrailleDisplay.show(rendered: Rendered): Unit =
    show(DisplayLine(rendered.braille, rendered.cursor, rendered.cells, rendered.cellCursor, rendered.before, rendered.after))
