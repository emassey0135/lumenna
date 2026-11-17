package dev.emassey0135.orbit

/**
 * What an app tells Orbit's screen reader about its screen, and what it reads of the reader's
 * settings: [OrbitReader]'s telling calls, apart from its binding, so that what is built over them
 * (`dev.emassey0135.orbit.ui`, [ReaderEditOutput]) can be tested with a reader that records.
 * Each call is described on [OrbitReader].
 */
interface Reader {
    /** The person's T9 commit timeout in milliseconds, or Orbit's default. */
    val characterCommitTimeout: Int

    /** The person's T9 commit feedback: 1 sound, 2 speak, 3 vibrate, 4 sound and speak, 5 vibrate and speak. */
    val characterCommitFeedback: Int

    /** Says [text], cutting off what was being said. */
    fun speak(text: String)

    /** Says [text] after what is being said. */
    fun speakQueued(text: String)

    fun stop()

    /** One of Orbit's sounds ([OrbitReader.SOUND_ERROR]…). */
    fun sound(id: Int)

    fun vibrate(milliseconds: Long)

    fun focusList(name: String, count: Int, selected: Int, item: String)

    fun itemChanged(name: String, count: Int, selected: Int, item: String)

    fun dialog(name: String, message: String, focused: String)

    fun dialogClosed()

    fun menuOpened()

    fun focusMenu(item: String, subMenu: Boolean, accessKey: Char?, position: Int, total: Int)

    /** [focusMenu] for the item [menu] has in hand. */
    fun focusMenu(menu: OrbitMenu<*>) {
        val item = menu.current ?: return
        focusMenu(item.name, item.isSubMenu, item.accessKey, menu.index + 1, menu.items.size)
    }

    fun contextMenuClosed(dismissed: Boolean = false)

    fun focusEdit(name: String, content: String, cursor: Int, line: String = content, multiline: Boolean = false)

    fun focusControl(name: String, role: Int)

    fun focusText(content: String)

    fun focusCheck(name: String, checked: Boolean)

    fun focusChoice(name: String, role: Int, count: Int, selected: Int, item: String)

    fun characterTyped(character: String, fieldName: String, capital: Boolean, resolvedWord: String?)

    fun editChanged(fieldName: String, content: String, cursor: Int, changeType: String)
}
