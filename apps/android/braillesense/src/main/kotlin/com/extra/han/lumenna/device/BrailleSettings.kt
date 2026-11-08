package com.extra.han.lumenna.device

import android.content.Context
import android.net.Uri

/**
 * The person's braille table, as the device has it in its settings provider (`BrailleCode`,
 * `ViewInputGrade`, `EightDotMode`), read when the app comes to the front: typing and the cells
 * follow the table they set in Global Options.
 */
class BrailleSettings(private val context: Context) {
    @Volatile var tables: String = BrailleTables.tables(BrailleTables.UEB, 2, false)
        private set

    @Volatile var eightDot: Boolean = false
        private set

    /** How long a message stays on the cells: the device's Message Display Time. */
    @Volatile var messageMillis: Long = 3_000L
        private set

    /** Reads the settings again; what cannot be read is left as it was. */
    fun refresh() {
        runCatching {
            val code = read("BrailleCode")?.toIntOrNull() ?: BrailleTables.UEB
            val grade = read("ViewInputGrade")?.toIntOrNull() ?: 2
            val eight = read("EightDotMode") == "1"
            eightDot = eight
            read("MessageDisplayTime")?.toLongOrNull()?.takeIf { it > 0 }?.let { messageMillis = it * 1_000L }
            tables = BrailleTables.tables(code, grade, eight)
        }
    }

    private fun read(name: String): String? = context.contentResolver.query(
        Uri.parse("content://com.selvashc.settingsprovider/braille_settings"), null, "name=?", arrayOf(name), null,
    )?.use { row -> if (row.moveToFirst()) row.getString(row.getColumnIndexOrThrow("value")) else null }
}

/** The device's braille codes, as liblouis tables. */
object BrailleTables {
    const val US = 0
    const val UEB = 1
    private val BY_GRADE: Map<Int, List<String>> = mapOf(
        // Each code's tables: grade 0 (computer), grade 1 and grade 2. A code the device has and
        // no table here falls to UEB at the same grade: better than nothing, and the person can
        // choose a table we have.
        US to listOf("en-us-comp8.ctb", "en-us-g1.ctb", "en-us-g2.ctb"),
        UEB to listOf("en-us-comp8.ctb", "en-ueb-g1.ctb", "en-ueb-g2.ctb"),
        2 to listOf("es.tbl", "es-g1.ctb", "es-g2.ctb"), // Spanish
        3 to listOf("es.tbl", "es-g1.ctb", "es-g2.ctb"), // Spanish (liblouis)
        4 to listOf("fr-bfu-comp8.utb", "fr-bfu-comp6.utb", "fr-bfu-g2.ctb"), // French unified
        5 to listOf("fr-bfu-comp8.utb", "fr-bfu-comp6.utb", "fr-bfu-g2.ctb"), // French
        6 to listOf("de-de-comp8.ctb", "de-g1.ctb", "de-g2.ctb"), // German
        7 to listOf("it-it-comp8.utb", "it-it-comp6.utb", "it-it-comp6.utb"), // Italian
        8 to listOf("en-gb-comp8.ctb", "en-gb-g1.utb", "en-GB-g2.ctb"), // UK
        9 to listOf("ar-ar-comp8.utb", "ar-ar-g1.utb", "ar-ar-g2.ctb"), // Arabic
        10 to listOf("da-dk-g08.ctb", "da-dk-g16.ctb", "da-dk-g26.ctb"), // Danish
        11 to listOf("da-dk-g08_1993.ctb", "da-dk-g16_1993.ctb", "da-dk-g26_1993.ctb"), // Danish 1993
        12 to listOf("sv-g0.utb", "sv-g1.ctb", "sv-g2.ctb"), // Swedish
        13 to listOf("pl-pl-comp8.ctb", "pl-pl-comp8.ctb", "pl-pl-comp8.ctb"), // Polish
        14 to listOf("pt-pt-comp8.ctb", "pt-pt-g1.utb", "pt-pt-g2.ctb"), // Portuguese
        15 to listOf("pt-pt-comp8.ctb", "pt-pt-g1.utb", "pt-pt-g2.ctb"), // Brazilian
        16 to listOf("nl-comp8.utb", "nl-NL-g0.utb", "nl-NL-g0.utb"), // Dutch
        17 to listOf("nl-comp8.utb", "nl-NL-g0.utb", "nl-NL-g0.utb"), // Belgian
        18 to listOf("ga-g1.utb", "ga-g1.utb", "ga-g2.ctb"), // Irish
        19 to listOf("he-IL-comp8.utb", "he-IL.utb", "he-IL.utb"), // Hebrew
        20 to listOf("ru-comp8.utb", "ru-ru-g1.ctb", "ru-ru-g1.ctb"), // Russian
        21 to listOf("hr-comp8.utb", "hr-g1.ctb", "hr-g1.ctb"), // Croatian
        22 to listOf("tr-g1.ctb", "tr-g1.ctb", "tr-g2.ctb"), // Turkish
        23 to listOf("el.ctb", "el.ctb", "el.ctb"), // Greek
        28 to listOf("lt-8dot.utb", "lt-6dot.utb", "lt-6dot.utb"), // Lithuanian
        32 to listOf("no-no-comp8.ctb", "no-no-g1.ctb", "no-no-g2.ctb"), // Norwegian
        33 to listOf("uk-comp.utb", "uk.utb", "uk.utb"), // Ukrainian
        36 to listOf("cs-comp8.utb", "cs-g1.ctb", "cs-g1.ctb"), // Czech
        38 to listOf("sk-sk.utb", "sk-sk-g1.utb", "sk-sk-g1.utb"), // Slovak
        39 to listOf("ro-g0.utb", "ro.ctb", "ro.ctb"), // Romanian
    )

    /** The table list for braille code [code] at [grade] (0 computer, 1, 2); computer braille in eight-dot mode. */
    fun tables(code: Int, grade: Int, eightDot: Boolean): String {
        val set = BY_GRADE[code] ?: BY_GRADE.getValue(UEB)
        val at = if (eightDot) 0 else grade.coerceIn(0, 2)
        return set[at]
    }
}
