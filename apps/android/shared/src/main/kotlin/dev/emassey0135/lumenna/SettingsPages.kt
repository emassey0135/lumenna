package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.SettingView
import dev.emassey0135.lumenna.app.SettingsPage
import dev.emassey0135.lumenna.app.settingsPages

/**
 * The core's settings pages (`settings_pages`), in sentence case, for the phone and the watch:
 * which settings each holds, its words and its buttons. Android's clock is the platform's, so
 * no page offers one.
 */
object SettingsPages {
    val all: List<SettingsPage> by lazy { settingsPages(true, true) }

    fun of(id: String): SettingsPage? = all.firstOrNull { it.id == id }

    /**
     * The pages worth opening, given [listed] (the settings the store lists here): a page with
     * a setting listed or a button. Before the settings come, every page but General, whose
     * settings are this device's own and need the store to say whether it lists any.
     */
    fun shown(listed: List<SettingView>): List<SettingsPage> {
        val keys = listed.map { it.setting.key }.toSet()
        return all.filter { page -> page.buttons.isNotEmpty() || page.keys.any { it in keys } || (keys.isEmpty() && page.id != "general") }
    }
}

/** [page]'s settings as the store lists them, in the page's order. */
fun List<SettingView>.on(page: SettingsPage?): List<SettingView> =
    page?.keys.orEmpty().mapNotNull { key -> firstOrNull { it.setting.key == key } }
