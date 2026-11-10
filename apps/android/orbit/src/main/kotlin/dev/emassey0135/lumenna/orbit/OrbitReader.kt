package dev.emassey0135.lumenna.orbit

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Bundle
import com.orbit.olympus_sdk.IOlympusSDKCallback
import com.orbit.screenreader.IScreenReaderService
import com.orbit.screenreader.ISettingsCallback
import com.orbit.screenreader.IUARTKeypadCallback
import dev.emassey0135.lumenna.braille.Claim
import dev.emassey0135.lumenna.braille.ServiceLink

/**
 * Orbit's screen reader: bound by explicit intent, every call in order on [ServiceLink]'s worker,
 * the keypad claimed under this app's own name while its window has focus.
 *
 * Rather than speak lines, this describes the screen ([focusList], [dialog], [contextMenu],
 * [focusEdit]…) and Orbit voices them in the person's own voice and verbosity. Until Orbit says
 * it is ready, what it is told it drops, so nothing is sent until then.
 *
 * Never #44 (`canSpeakLanguage`), which the Player lacks: the AIDL here stops before it.
 */
class OrbitReader(context: Context, private val listener: Listener) {
    interface Listener {
        /** Orbit said it is ready, on a binder thread: the screen is described now. */
        fun readerReady()

        /** A key, on a binder thread: whether it was taken. */
        fun key(event: KeyEvent, phoneKeypad: Boolean): Boolean

        /** An Orbit setting changed (the braille input table); read what depends on it again. */
        fun settingsChanged()
    }

    private val packageName = context.packageName

    @Volatile
    private var ready = false

    @Volatile
    private var phoneKeypad = false

    /** Whether the keys come from the Player's phone keypad rather than braille keys. */
    val isPhoneKeypad: Boolean get() = phoneKeypad

    private val sdk = object : IOlympusSDKCallback.Stub() {
        override fun onScreenReaderReady() {
            ready = true
            claim.serviceUp(true)
            listener.readerReady()
        }

        override fun onUtteranceStart(utteranceId: String?) {}
        override fun onUtteranceDone(utteranceId: String?) {}
        override fun onUtteranceError(utteranceId: String?) {}
    }

    private val keys = object : IUARTKeypadCallback.Stub() {
        override fun onInternalKeyEvent(event: Bundle): Boolean {
            val code = event.getInt("keyCode", -1)
            if (code in 1000..1022) phoneKeypad = true else if (code >= 3000) phoneKeypad = false
            val name = event.getString("keyName") ?: return claim.holding
            return listener.key(KeyEvent(name, event.getInt("action", -1), event.getBoolean("isLongPress", false)), phoneKeypad)
        }

        override fun onAdjustVolume(increase: Boolean) {}
    }

    private val settings = object : ISettingsCallback.Stub() {
        override fun onSettingsChanged() = listener.settingsChanged()
    }

    private val link: ServiceLink<IScreenReaderService> = ServiceLink(
        context,
        Intent("com.orbit.screenreader.IScreenReaderService")
            .setComponent(ComponentName("com.orbit.screenreader", "com.orbit.screenreader.service.ScreenReaderBoundService")),
        { IScreenReaderService.Stub.asInterface(it) },
        connected = { service ->
            service.registerSDKCallback(sdk)
            service.registerSettingsCallback(settings)
            service.registerKeypadCallback(packageName, keys)
            commitTimeout = runCatching { service.characterCommitTimeout }.getOrDefault(0)
            commitFeedback = runCatching { service.characterCommitFeedback }.getOrDefault(1)
        },
        lost = {
            ready = false
            claim.serviceUp(false)
        },
    )

    /** Taken while the window has focus: see [Claim]. */
    val claim: Claim = Claim(
        take = { link.call("notifyAppForeground") { it.notifyAppForeground(packageName) } },
        release = { link.now("notifyAppBackground") { it.notifyAppBackground(packageName) } },
        needsService = true,
    )

    @Volatile
    private var commitTimeout = 0

    @Volatile
    private var commitFeedback = 1

    /** The person's T9 commit timeout in milliseconds, or Orbit's default. */
    val characterCommitTimeout: Int get() = commitTimeout.takeIf { it > 0 } ?: 500

    /** The person's T9 commit feedback: 1 sound, 2 speak, 3 vibrate, 4 sound and speak, 5 vibrate and speak. */
    val characterCommitFeedback: Int get() = commitFeedback

    fun bind() = link.bind()

    fun unbind() {
        link.now("unregisterKeypadCallback") { it.unregisterKeypadCallback(packageName) }
        link.now("unregisterSettingsCallback") { it.unregisterSettingsCallback(settings) }
        link.unbind()
    }

    val isReady: Boolean get() = ready

    private fun tell(what: String, block: (IScreenReaderService) -> Unit) {
        if (ready) link.call(what, block)
    }

    fun speak(text: String) = tell("speakText") {
        it.stopSpeaking()
        it.speakText(text)
    }

    fun speakQueued(text: String) = tell("speakText") { it.speakText(text) }

    fun stop() = tell("stopSpeaking") { it.stopSpeaking() }

    fun sound(id: Int) = tell("playSound") { it.playSound(id) }

    fun vibrate(milliseconds: Long) = tell("vibrate") { it.vibrate(milliseconds) }

    fun appOpened(name: String, focused: String) {
        val bundle = Bundle().apply {
            putString("appName", name)
            putString("focusedElement", focused)
        }
        tell("onAppOpened") { it.onAppOpened(bundle) }
    }

    /** One description of the screen ([describe]). */
    fun perform(call: ReaderCall) = when (call) {
        is ReaderCall.FocusList -> focusList(call.name, call.count, call.selected, call.item)
        is ReaderCall.ItemChanged -> itemChanged(call.name, call.count, call.selected, call.item)
        is ReaderCall.Dialog -> dialog(call.name, "", call.focused)
        is ReaderCall.ContextMenu -> contextMenu(call.name, "")
        is ReaderCall.Say -> speakQueued(call.text)
    }

    /** Lists are said as list boxes (role 5), the name the list's, the selected the row in hand. */
    fun focusList(name: String, count: Int, selected: Int, item: String) =
        tell("onFocusChanged") { it.onFocusChanged(listBundle(name, count, selected, item)) }

    fun itemChanged(name: String, count: Int, selected: Int, item: String) =
        tell("onItemChanged") { it.onItemChanged(listBundle(name, count, selected, item)) }

    private fun listBundle(name: String, count: Int, selected: Int, item: String, role: Int = ROLE_LIST_BOX): Bundle =
        Bundle().apply {
            putString("name", name)
            putInt("roleId", role)
            putInt("itemCount", count)
            putInt("selectedIndex", if (count == 0) -1 else selected)
            if (count > 0) putString("selectedItem", item)
        }

    fun dialog(name: String, message: String, focused: String) = tell("onDialogOpened") {
        it.onDialogOpened(Bundle().apply {
            putString("name", name)
            putString("message", message)
            putString("focusedElement", focused)
        })
    }

    fun dialogClosed() = tell("onDialogClosed") { it.onDialogClosed(Bundle()) }

    fun contextMenu(name: String, instructions: String) = tell("onContextMenuOpened") {
        it.onContextMenuOpened(Bundle().apply {
            putString("name", name)
            putString("instructions", instructions)
        })
    }

    fun contextMenuClosed() = tell("onContextMenuClosed") { it.onContextMenuClosed(Bundle()) }

    /** An edit box (role 6), so Orbit says the field and its review commands read its text. */
    fun focusEdit(name: String, content: String, cursor: Int) = tell("onFocusChanged") {
        it.onFocusChanged(Bundle().apply {
            putString("name", name)
            putInt("roleId", ROLE_EDIT_BOX)
            putString("content", content)
            putString("currentLine", content)
            putInt("cursorPosition", cursor)
            putBoolean("isMultiline", false)
            putBoolean("isReadonly", false)
        })
    }

    /** A button (role 1), a read-only line (role 7): Orbit says its name and what it is. */
    fun focusControl(name: String, role: Int) = tell("onFocusChanged") {
        it.onFocusChanged(Bundle().apply {
            putString("name", name)
            putInt("roleId", role)
        })
    }

    /** A check box (role 2) and whether it is checked. */
    fun focusCheck(name: String, checked: Boolean) = tell("onFocusChanged") {
        it.onFocusChanged(Bundle().apply {
            putString("name", name)
            putInt("roleId", ROLE_CHECK_BOX)
            putBoolean("checked", checked)
            putInt("stateId", if (checked) 1 else 0)
        })
    }

    /** A combo box (role 4) or a list inside a dialog, by [role], and what is chosen. */
    fun focusChoice(name: String, role: Int, count: Int, selected: Int, item: String) =
        tell("onFocusChanged") { it.onFocusChanged(listBundle(name, count, selected, item, role)) }

    /** A character typed, echoed as Orbit's edit fields do. */
    fun characterTyped(character: String, fieldName: String, capital: Boolean, resolvedWord: String?) = tell("onCharacterTyped") {
        it.onCharacterTyped(Bundle().apply {
            putString("character", character)
            putString("characterName", fieldName)
            putBoolean("isCapital", capital)
            if (resolvedWord != null) putString("resolvedWord", resolvedWord)
        })
    }

    /** An edit box's new content; Orbit says what changed by [changeType], as its own fields do. */
    fun editChanged(fieldName: String, content: String, cursor: Int, changeType: String) = tell("onEditBoxContentChanged") {
        it.onEditBoxContentChanged(Bundle().apply {
            putString("widgetName", fieldName)
            putString("content", content)
            putInt("cursorPosition", cursor)
            putString("changeType", changeType)
        })
    }

    companion object {
        const val ROLE_BUTTON = 1
        const val ROLE_CHECK_BOX = 2
        const val ROLE_COMBO_BOX = 4
        const val ROLE_LIST_BOX = 5
        const val ROLE_EDIT_BOX = 6
        const val ROLE_TEXT_BOX = 7
        const val SOUND_ERROR = 1
        const val SOUND_COMMIT = 9
    }
}
