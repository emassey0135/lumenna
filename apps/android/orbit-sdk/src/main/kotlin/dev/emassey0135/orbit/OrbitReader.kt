package dev.emassey0135.orbit

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Bundle
import com.orbit.olympus_sdk.IOlympusSDKCallback
import com.orbit.screenreader.IScreenReaderService
import com.orbit.screenreader.ISettingsCallback
import com.orbit.screenreader.IUARTKeypadCallback

/**
 * Orbit's screen reader: bound by explicit intent, every call in order on [ServiceLink]'s worker,
 * the keypad claimed under this app's own name while its window has focus.
 *
 * Rather than speak lines, an app describes its screen ([focusList], [dialog], [contextMenu],
 * [focusEdit]…) and Orbit voices it in the person's own voice and verbosity, as it voices Orbit's
 * own apps. Until Orbit says it is ready, what it is told it drops, so nothing is sent until then.
 *
 * Orbit routes its keypad to an app (rather than reading its window through accessibility) only
 * when the app says it speaks for itself: Satellite's hook does so for an app whose manifest has
 * `orbit.self_voicing`, and Orbit's own apps are routed by their `com.orbit.` package.
 *
 * Never #44 (`canSpeakLanguage`), which the Player lacks: the AIDL here stops before it.
 */
public class OrbitReader(context: Context, private val listener: Listener) : Reader {
    /** What Orbit tells the app, each on a binder thread. */
    public interface Listener {
        /** Orbit said it is ready, on a binder thread: the screen is described now. */
        public fun readerReady()

        /** A key, on a binder thread: whether it was taken. */
        public fun key(event: KeyEvent, phoneKeypad: Boolean): Boolean

        /** An Orbit setting changed (the braille input table); read what depends on it again. */
        public fun settingsChanged()
    }

    private val packageName = context.packageName

    @Volatile
    private var ready = false

    @Volatile
    private var phoneKeypad = false

    /** Whether the keys come from the Player's phone keypad rather than braille keys. */
    public val isPhoneKeypad: Boolean get() = phoneKeypad

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
    public val claim: Claim = Claim(
        take = { link.call("notifyAppForeground") { it.notifyAppForeground(packageName) } },
        release = { link.now("notifyAppBackground") { it.notifyAppBackground(packageName) } },
        needsService = true,
    )

    @Volatile
    private var commitTimeout = 0

    @Volatile
    private var commitFeedback = 1

    /** The person's T9 commit timeout in milliseconds, or Orbit's default. */
    override val characterCommitTimeout: Int get() = commitTimeout.takeIf { it > 0 } ?: 500

    /** The person's T9 commit feedback: 1 sound, 2 speak, 3 vibrate, 4 sound and speak, 5 vibrate and speak. */
    override val characterCommitFeedback: Int get() = commitFeedback

    /** Reaches Orbit's screen reader; [Listener.readerReady] follows once it is ready. */
    public fun bind(): Unit = link.bind()

    /** Gives the keypad back and lets the reader go. */
    public fun unbind() {
        link.now("unregisterKeypadCallback") { it.unregisterKeypadCallback(packageName) }
        link.now("unregisterSettingsCallback") { it.unregisterSettingsCallback(settings) }
        link.unbind()
    }

    /** Whether Orbit's reader has said it is ready: nothing told before then is said. */
    public val isReady: Boolean get() = ready

    private fun tell(what: String, block: (IScreenReaderService) -> Unit) {
        if (ready) link.call(what, block)
    }

    override fun speak(text: String): Unit = tell("speakText") {
        it.stopSpeaking()
        it.speakText(text)
    }

    override fun speakQueued(text: String): Unit = tell("speakText") { it.speakText(text) }

    override fun stop(): Unit = tell("stopSpeaking") { it.stopSpeaking() }

    override fun sound(id: Int): Unit = tell("playSound") { it.playSound(id) }

    override fun vibrate(milliseconds: Long): Unit = tell("vibrate") { it.vibrate(milliseconds) }

    /** The app [name] opened, [focused] what is in hand: Orbit says the app's name. */
    public fun appOpened(name: String, focused: String) {
        val bundle = Bundle().apply {
            putString("appName", name)
            putString("focusedElement", focused)
        }
        tell("onAppOpened") { it.onAppOpened(bundle) }
    }

    /** Lists are said as list boxes (role 5), the name the list's, the selected the row in hand. */
    override fun focusList(name: String, count: Int, selected: Int, item: String): Unit =
        tell("onFocusChanged") { it.onFocusChanged(listBundle(name, count, selected, item)) }

    override fun itemChanged(name: String, count: Int, selected: Int, item: String): Unit =
        tell("onItemChanged") { it.onItemChanged(listBundle(name, count, selected, item)) }

    private fun listBundle(name: String, count: Int, selected: Int, item: String, role: Int = ROLE_LIST_BOX): Bundle =
        Bundle().apply {
            putString("name", name)
            putInt("roleId", role)
            putInt("itemCount", count)
            putInt("selectedIndex", if (count == 0) -1 else selected)
            if (count > 0) putString("selectedItem", item)
        }

    override fun dialog(name: String, message: String, focused: String): Unit = tell("onDialogOpened") {
        it.onDialogOpened(Bundle().apply {
            putString("name", name)
            putString("message", message)
            putString("focusedElement", focused)
        })
    }

    override fun dialogClosed(): Unit = tell("onDialogClosed") { it.onDialogClosed(Bundle()) }

    /** A context menu named [name], [instructions] said after it; [menuOpened] leaves the naming to Orbit. */
    public fun contextMenu(name: String, instructions: String): Unit = tell("onContextMenuOpened") {
        it.onContextMenuOpened(Bundle().apply {
            putString("name", name)
            putString("instructions", instructions)
        })
    }

    override fun contextMenuClosed(dismissed: Boolean): Unit = tell("onContextMenuClosed") {
        it.onContextMenuClosed(Bundle().apply { putBoolean("dismissed", dismissed) })
    }

    /**
     * A menu of Orbit's own kind ([OrbitMenu]) opened, as its apps open theirs: no name, so Orbit
     * says its own "Context menu" in the person's language.
     */
    override fun menuOpened(): Unit = tell("onContextMenuOpened") { it.onContextMenuOpened(Bundle()) }

    /**
     * The item in hand of a menu (role 8), as Orbit's apps tell each move of theirs: Orbit says
     * its name, "Sub-menu" for a submenu, its access key where the person has Orbit say them, and
     * its place ("2 of 7") where they have it say that.
     */
    override fun focusMenu(item: String, subMenu: Boolean, accessKey: Char?, position: Int, total: Int): Unit = tell("onFocusChanged") {
        it.onFocusChanged(Bundle().apply {
            putString("name", item)
            putInt("roleId", ROLE_MENU)
            putString("selectedItem", item)
            putString("menuName", item)
            putBoolean("isSubMenu", subMenu)
            putString("accessKey", accessKey?.toString().orEmpty())
            putInt("positionCurrent", position)
            putInt("positionTotal", total)
        })
    }

    /** An edit box (role 6), so Orbit says the field and its review commands read its text. */
    override fun focusEdit(name: String, content: String, cursor: Int, line: String, multiline: Boolean): Unit = tell("onFocusChanged") {
        it.onFocusChanged(Bundle().apply {
            putString("name", name)
            putInt("roleId", ROLE_EDIT_BOX)
            putString("content", content)
            putString("currentLine", line)
            putInt("cursorPosition", cursor)
            // Orbit then says "Multiline" itself, as of its own fields.
            putBoolean("isMultiline", multiline)
            putBoolean("isReadonly", false)
        })
    }

    /** A button (role 1): Orbit says its name and what it is. */
    override fun focusControl(name: String, role: Int): Unit = tell("onFocusChanged") {
        it.onFocusChanged(Bundle().apply {
            putString("name", name)
            putInt("roleId", role)
        })
    }

    /** A read-only line (role 7): Orbit reads its content, which is all there is of it. */
    override fun focusText(content: String): Unit = tell("onFocusChanged") {
        it.onFocusChanged(Bundle().apply {
            putString("name", "")
            putInt("roleId", ROLE_TEXT_BOX)
            putString("content", content)
            putBoolean("isReadonly", true)
        })
    }

    /** A check box (role 2) and whether it is checked. */
    override fun focusCheck(name: String, checked: Boolean): Unit = tell("onFocusChanged") {
        it.onFocusChanged(Bundle().apply {
            putString("name", name)
            putInt("roleId", ROLE_CHECK_BOX)
            putBoolean("checked", checked)
            putInt("stateId", if (checked) 1 else 0)
        })
    }

    /** A combo box (role 4) or a list inside a dialog, by [role], and what is chosen. */
    override fun focusChoice(name: String, role: Int, count: Int, selected: Int, item: String): Unit =
        tell("onFocusChanged") { it.onFocusChanged(listBundle(name, count, selected, item, role)) }

    /** A character typed, echoed as Orbit's edit fields do. */
    override fun characterTyped(character: String, fieldName: String, capital: Boolean, resolvedWord: String?): Unit = tell("onCharacterTyped") {
        it.onCharacterTyped(Bundle().apply {
            putString("character", character)
            putString("characterName", fieldName)
            putBoolean("isCapital", capital)
            if (resolvedWord != null) putString("resolvedWord", resolvedWord)
        })
    }

    /** An edit box's new content; Orbit says what changed by [changeType], as its own fields do. */
    override fun editChanged(fieldName: String, content: String, cursor: Int, changeType: String): Unit = tell("onEditBoxContentChanged") {
        it.onEditBoxContentChanged(Bundle().apply {
            putString("widgetName", fieldName)
            putString("content", content)
            putInt("cursorPosition", cursor)
            putString("changeType", changeType)
        })
    }

    /** Orbit's roles of what is focused, and its sounds. */
    public companion object {
        /** A button. */
        public const val ROLE_BUTTON: Int = 1
        /** A check box. */
        public const val ROLE_CHECK_BOX: Int = 2
        /** A combo box. */
        public const val ROLE_COMBO_BOX: Int = 4
        /** A list box. */
        public const val ROLE_LIST_BOX: Int = 5
        /** An edit box. */
        public const val ROLE_EDIT_BOX: Int = 6
        /** A read-only line. */
        public const val ROLE_TEXT_BOX: Int = 7
        /** An item of a menu. */
        public const val ROLE_MENU: Int = 8
        /** Orbit's error tone. */
        public const val SOUND_ERROR: Int = 1
        /** Orbit's sound of a T9 character committed. */
        public const val SOUND_COMMIT: Int = 9
    }
}
