package dev.emassey0135.lumenna.orbit

/** The keys of this device, each from the core's words. */
fun keyHelpLines(phoneKeypad: Boolean): List<String> = if (phoneKeypad) {
    listOf("up and down move", "ok opens", "menu shows actions", "back goes back", "digits open a row by number, star held finds by letter")
} else {
    listOf("up and down move", "dot 8 or ok opens", "dot 7 goes back", "space with m shows actions", "space with a letter runs its command", "a letter alone finds a row")
}
