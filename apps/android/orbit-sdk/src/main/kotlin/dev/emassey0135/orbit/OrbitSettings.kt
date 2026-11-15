package dev.emassey0135.orbit

/**
 * The settings of Orbit's that change how an app reads and types, each read through [Root] as Orbit's
 * own apps read them. Off the main thread.
 */
object OrbitSettings {
    /** How the person types: Orbit's selection mode and the input table it names. */
    data class BrailleInput(
        /** The liblouis table file, as the liblouis service takes it: "en-ueb-g2.ctb". */
        val table: String,
        val mode: SelectionMode,
    )

    /** Orbit's braille selection mode, which the screen reader's G chord changes. */
    enum class SelectionMode { CONTRACTED, UNCONTRACTED, COMPUTER }

    /** What the screen reader echoes as the person types, its `typing_echo`. */
    enum class TypingEcho(val value: Int) {
        OFF(0), CHARACTERS(1), WORDS(2), BOTH(3);

        val characters: Boolean get() = this == CHARACTERS || this == BOTH
        val words: Boolean get() = this == WORDS || this == BOTH

        companion object {
            fun of(value: Int): TypingEcho = entries.firstOrNull { it.value == value } ?: CHARACTERS
        }
    }

    /** The person's selection mode and its input table, from Orbit's settings provider. */
    fun brailleInput(): Result<BrailleInput> = Root.run(
        KEYS.joinToString("") { "echo @@$it; content query --uri $SETTINGS$it; " },
    ).map { output ->
        val values = parseMarked(output)
        val named = values["key_selection_mode"]?.lowercase()
        val mode = when {
            named == null -> SelectionMode.CONTRACTED
            "uncontract" in named -> SelectionMode.UNCONTRACTED
            "computer" in named -> SelectionMode.COMPUTER
            else -> SelectionMode.CONTRACTED
        }
        val key = when (mode) {
            SelectionMode.CONTRACTED -> "key_contracted_braille_input_table"
            SelectionMode.UNCONTRACTED -> "key_uncontracted_braille_input_table"
            SelectionMode.COMPUTER -> "key_computer_braille_input_table"
        }
        BrailleInput(values[key]?.takeIf { it.isNotEmpty() } ?: DEFAULT_TABLE, mode)
    }

    /** The screen reader's typing echo, from its own settings file. */
    fun typingEcho(): Result<TypingEcho> = Root.run("cat ${Root.dataPath(READER_SETTINGS)}").mapCatching { output ->
        val value = Regex("name=\"typing_echo\" value=\"(\\d+)\"").find(output)?.groupValues?.get(1)?.toIntOrNull()
            ?: throw RootFailure("no typing_echo in the screen reader's settings")
        TypingEcho.of(value)
    }

    /** The table Orbit's English build starts with. */
    const val DEFAULT_TABLE = "en-ueb-g2.ctb"

    private const val SETTINGS = "content://com.orbit.settings.provider/preferences/"
    // Another app's data, read through root at Root.dataPath: never this app's own files.
    @Suppress("SdCardPath")
    private const val READER_SETTINGS = "/data/data/com.orbit.screenreader/shared_prefs/screenreader_general_settings.xml"
    private val KEYS = listOf(
        "key_selection_mode",
        "key_contracted_braille_input_table",
        "key_uncontracted_braille_input_table",
        "key_computer_braille_input_table",
    )

    /** Each key's value from the marked output of several queries; tested on the JVM. */
    internal fun parseMarked(output: String): Map<String, String> {
        val values = mutableMapOf<String, String>()
        for (part in output.split("@@")) {
            val newline = part.indexOf('\n')
            if (newline < 0) continue
            Root.valueField(part, newline)?.let { values[part.substring(0, newline).trim()] = it }
        }
        return values
    }
}
