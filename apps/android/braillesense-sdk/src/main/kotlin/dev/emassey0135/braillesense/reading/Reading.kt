package dev.emassey0135.braillesense.reading

/**
 * How the BrailleSense reads a control, as its own apps' controls do (its SDK's `HanListView`,
 * `HanEditText` and the rest, and the built-in apps): a role with a word for the voice and a
 * symbol for the cells, placed where the person's Control Information settings put it, the
 * shortcut letter in the code's brackets, a hot key, a row's place among several, and the sign of
 * what pressing it opens. The words are the device's own, in English.
 */
enum class ControlRole(
    /** What the voice calls it. */
    val spoken: String,
    /** Its symbol on the cells, shown as Braille ASCII. */
    val symbol: String,
) {
    /** Opens a submenu. */
    MENU("Menu", "mn"),
    /** Runs when pressed. */
    MENU_ITEM("menu item", "mi"),
    /** A row of a list. */
    LIST_ITEM("list item", "li"),
    /** A line to type in. */
    EDIT_BOX("edit box", "eb"),
    /** Text of several lines to type in, where Enter starts a new line. */
    MULTI_LINE_EDIT_BOX("multi line edit box", "meb"),
    /** One of several, changed in place. */
    COMBO_BOX("combo box", "cb"),
    /** A check box that is checked. */
    CHECK_BOX_CHECKED("check box checked", "schb"),
    /** A check box that is not. */
    CHECK_BOX_UNCHECKED("check box unchecked", "uchb"),
    /** Runs when pressed. */
    BUTTON("button", "bt"),
    /** An answer to a question, chosen in place. */
    PROMPT_BUTTON("prompt button", "pb"),
    /** A line of information. */
    STATIC_BOX("static box", "st"),
}

/** Where a role goes on its line: Control Information, one setting for the cells and one for the voice. */
enum class RolePlace { OFF, BEFORE, AFTER }

/** What the person's Global Options say a line carries. */
data class LineSettings(
    /** Where the voice says the role. */
    val voiceRole: RolePlace,
    /** Where the cells show it. */
    val brailleRole: RolePlace,
    /** Whether the voice says a shortcut letter. */
    val voiceLetter: Boolean,
    /** Whether the cells show it. */
    val brailleLetter: Boolean,
    /** Whether the voice says a hot key. */
    val voiceHotKey: Boolean,
    /** Whether it says it as dots rather than a letter. */
    val voiceHotKeyDots: Boolean,
    /** Whether the cells show it. */
    val brailleHotKey: Boolean,
    /** Whether a place among several is given: "2/5". */
    val index: Boolean,
)

/** The cells of the device's own signs in its braille code, as Braille ASCII. */
data class BrailleSigns(
    /** What opens a shortcut letter. */
    val open: String,
    /** What closes it. */
    val close: String,
    /** What ends an item that opens a dialog. */
    val dialog: String,
    /** What ends an item that opens a submenu. */
    val pullDown: String,
)

/** One line to read. */
data class DeviceLine(
    val role: ControlRole,
    /** What it is for, ahead of its value; empty for none. */
    val label: String = "",
    /** Its text as said. */
    val speech: String,
    /** Its text as shown. */
    val text: String = speech,
    /** What follows the letter, said and shown; empty for none. */
    val detail: String = "",
    /** Its shortcut letter. */
    val letter: String? = null,
    /** What runs it from anywhere: "Enter-n". */
    val hotKey: String? = null,
    /** Whether it opens a dialog. */
    val asks: Boolean = false,
    /** Which of [count] it is, from 1, for a row or a control that is one of several. */
    val position: Int = 0,
    /** How many there are; 0 for a line that is not one of several. */
    val count: Int = 0,
)

/** A stretch of the cells beside a line's text. */
data class BraillePart(
    val text: String,
    /** Whether it is Braille ASCII, shown cell for cell, rather than text in the person's table. */
    val computer: Boolean,
)

/** A line as the device reads it. */
data class DeviceReading(
    /** What is said. */
    val speech: String,
    /** What the cells show ahead of [body]. */
    val before: List<BraillePart>,
    /** The text, in the person's table; an edit box puts its own cells here. */
    val body: String,
    /** What the cells show after it. */
    val after: List<BraillePart>,
)

/**
 * A line in the device's own order: the role where the settings put it, the name, the shortcut
 * letter in brackets, a hot key, and the sign of what pressing it opens.
 */
fun deviceReading(line: DeviceLine, settings: LineSettings, signs: BrailleSigns): DeviceReading {
    val index = if (settings.index && line.count > 0) "${line.position}/${line.count}" else null

    val speech = StringBuilder()
    if (line.label.isNotEmpty()) {
        speech.append(line.label)
        if (line.speech.isNotEmpty()) speech.append(": ")
    }
    speech.append(line.speech)
    if (settings.voiceLetter && line.letter != null) speech.append(" (${line.letter})")
    if (line.detail.isNotEmpty()) speech.append(", ").append(line.detail)
    if (index != null) speech.append(' ').append(index)
    // The device says "dialog" ahead of an item's hot key, and "pulldown" after a submenu's.
    if (line.role != ControlRole.MENU && line.asks) speech.append(" dialog")
    if (settings.voiceHotKey && line.hotKey != null) {
        speech.append(' ').append(if (settings.voiceHotKeyDots) hotKeyDots(line.hotKey) else line.hotKey)
    }
    if (line.role == ControlRole.MENU) speech.append(" pulldown")
    val said = when (settings.voiceRole) {
        RolePlace.OFF -> speech.toString()
        RolePlace.BEFORE -> "${line.role.spoken} $speech"
        RolePlace.AFTER -> "$speech ${line.role.spoken}"
    }

    val before = mutableListOf<BraillePart>()
    if (settings.brailleRole == RolePlace.BEFORE) before += BraillePart("${line.role.symbol} ", computer = true)
    if (line.label.isNotEmpty()) before += BraillePart("${line.label}: ", computer = false)
    val after = mutableListOf<BraillePart>()
    if (settings.brailleLetter && line.letter != null) after += BraillePart(" ${signs.open}${line.letter}${signs.close}", computer = true)
    if (line.detail.isNotEmpty()) after += BraillePart(" ${line.detail}", computer = false)
    if (index != null) after += BraillePart(" $index", computer = false)
    if (settings.brailleHotKey && line.hotKey != null) {
        // Its name on the cells is the device's short one: "ent-n".
        after += BraillePart(" " + line.hotKey.replace("Enter", "ent").replace("Space", "sp").lowercase(), computer = true)
    }
    if (line.role == ControlRole.MENU) {
        after += BraillePart(signs.pullDown, computer = true)
    } else if (line.asks) {
        after += BraillePart(" ${signs.dialog}", computer = true)
    }
    if (settings.brailleRole == RolePlace.AFTER) after += BraillePart(" ${line.role.symbol}", computer = true)
    return DeviceReading(said, before, line.text, after)
}

/** What opening a dialog says: its title and that it is one. */
fun dialogTitle(title: String): String = "$title dialog"

/** A hot key named by its letter, as dots: "Enter-n" is "Enter-dots 1-3-4-5", "Space-a" "Space-dot 1". */
internal fun hotKeyDots(key: String): String {
    val at = key.lastIndexOf('-')
    if (at < 0) return key
    val held = key.substring(0, at)
    val letter = key.substring(at + 1).singleOrNull()?.takeIf { it in 'a'..'z' } ?: return key
    val dots = LETTER_DOTS[letter - 'a']
    return "$held-${if (dots.length == 1) "dot" else "dots"} ${dots.toList().joinToString("-")}"
}

/** The dots of a to z. */
internal val LETTER_DOTS = listOf(
    "1", "12", "14", "145", "15", "124", "1245", "125", "24", "245", "13", "123", "134",
    "1345", "135", "1234", "12345", "1235", "234", "2345", "136", "1236", "2456", "1346", "13456", "1356",
)
