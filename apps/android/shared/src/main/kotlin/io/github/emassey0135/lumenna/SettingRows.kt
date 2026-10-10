package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.core.Setting
import io.github.emassey0135.lumenna.core.SettingKind

// Settings as the phone's and the watch's screens show them. What each is called, how its
// value is shaped, its options and what it does are the core's (`Setting`); which page it
// goes on is the app's.

/** Every setting, in the core's order; none when the store cannot say. */
fun settings(core: Core): List<Setting> = core.attempt { core.lumenna.settings(null).settings }.orEmpty()

/** The Planning page's: whatever syncs to every device. */
fun List<Setting>.planning(): List<Setting> = filter { it.syncs }

/**
 * The Backups page's: this device's own backup settings, but for where they go, which on
 * Android is the app's own storage and not a folder to choose.
 */
fun List<Setting>.backups(): List<Setting> = filter { !it.syncs && it.key.startsWith("backup-") && it.kind != SettingKind.FOLDER }

/** A setting's value as it is said: its option's name, a time in this device's clock. */
val Setting.said: String
    get() = when (kind) {
        SettingKind.TIME -> Clock.time(value)
        SettingKind.TOGGLE, SettingKind.CHOICE -> options.firstOrNull { it.id == value }?.title ?: value
        else -> value
    }

/** Whether a toggle is on. */
val Setting.on: Boolean get() = value == "true"

/** Changes a setting, saying what happened. */
fun Core.set(setting: Setting, value: String) = change { it.setSetting(setting.key, value) }
