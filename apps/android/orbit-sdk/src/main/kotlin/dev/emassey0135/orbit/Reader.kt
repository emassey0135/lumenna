package dev.emassey0135.orbit

/**
 * What an app tells Orbit's screen reader about its screen, and what it reads of the reader's
 * settings: [OrbitReader]'s telling calls, apart from its binding, so that what is built over them
 * (`dev.emassey0135.orbit.ui`, [ReaderEditOutput]) can be tested with a reader that records.
 * Each call is described on [OrbitReader].
 */
public interface Reader {
    /** The person's T9 commit timeout in milliseconds, or Orbit's default. */
    public val characterCommitTimeout: Int

    /** The person's T9 commit feedback: 1 sound, 2 speak, 3 vibrate, 4 sound and speak, 5 vibrate and speak. */
    public val characterCommitFeedback: Int

    /** Says [text], cutting off what was being said. */
    public fun speak(text: String)

    /** Says [text] after what is being said. */
    public fun speakQueued(text: String)

    /** Stops what is being said. */
    public fun stop()

    /** One of Orbit's sounds ([OrbitReader.SOUND_ERROR]…). */
    public fun sound(id: Int)

    /** Vibrates for [milliseconds]. */
    public fun vibrate(milliseconds: Long)

    /** A list named [name] reached, [count] rows, the row [selected] (from 0) in hand reading [item]. */
    public fun focusList(name: String, count: Int, selected: Int, item: String)

    /** The row in hand of the list [name] moved to [selected], reading [item]. */
    public fun itemChanged(name: String, count: Int, selected: Int, item: String)

    /** A dialog named [name] opened, [message] said with it and [focused] in hand. */
    public fun dialog(name: String, message: String, focused: String)

    /** The dialog open closed. */
    public fun dialogClosed()

    /** A menu opened, unnamed: Orbit says its own "Context menu". */
    public fun menuOpened()

    /** An item of the menu open in hand: [item], whether it is a [subMenu], its [accessKey], and its place. */
    public fun focusMenu(item: String, subMenu: Boolean, accessKey: Char?, position: Int, total: Int)

    /** [focusMenu] for the item [menu] has in hand. */
    public fun focusMenu(menu: OrbitMenu<*>) {
        val item = menu.current ?: return
        focusMenu(item.name, item.isSubMenu, item.accessKey, menu.index + 1, menu.items.size)
    }

    /** The menu open closed: [dismissed] unchosen. */
    public fun contextMenuClosed(dismissed: Boolean = false)

    /** An edit box named [name] in hand, holding [content] with the caret at [cursor] on [line]. */
    public fun focusEdit(name: String, content: String, cursor: Int, line: String = content, multiline: Boolean = false)

    /** A control named [name] of [role] (the OrbitReader roles) in hand. */
    public fun focusControl(name: String, role: Int)

    /** A line to read, [content], in hand. */
    public fun focusText(content: String)

    /** A check box named [name] in hand, [checked] or not. */
    public fun focusCheck(name: String, checked: Boolean)

    /** A combo box or list box named [name] in hand, [item] chosen of [count]. */
    public fun focusChoice(name: String, role: Int, count: Int, selected: Int, item: String)

    /** A [character] typed in [fieldName]; Orbit echoes it by the person's settings. */
    public fun characterTyped(character: String, fieldName: String, capital: Boolean, resolvedWord: String?)

    /** The edit box [fieldName] changed to [content], by [changeType]. */
    public fun editChanged(fieldName: String, content: String, cursor: Int, changeType: String)
}
