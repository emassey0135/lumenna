package dev.emassey0135.lumenna.braille

import dev.emassey0135.lumenna.app.Choice
import dev.emassey0135.lumenna.app.ControlMove
import dev.emassey0135.lumenna.app.ControlSends
import dev.emassey0135.lumenna.app.ControlShown
import dev.emassey0135.lumenna.app.ControlType
import dev.emassey0135.lumenna.app.DialogShown
import dev.emassey0135.lumenna.app.ShellEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DialogControllerTest {
    private val sent = mutableListOf<String>()
    private val shelled = mutableListOf<String>()

    /** What the core's `control_event` would send, written out so a test can read it. */
    private fun events(sends: ControlSends, value: String, how: ControlMove): List<ShellEvent> {
        val what = when (sends) {
            is ControlSends.TaskField -> "field ${sends.field0}"
            is ControlSends.Setting -> "setting ${sends.field0}"
            is ControlSends.QuickAdd -> "quick-add"
            is ControlSends.Answer -> if (how is ControlMove.Pressed) "answer" else return emptyList()
            is ControlSends.Pick -> "pick"
            is ControlSends.Press -> return if (how is ControlMove.Pressed) sends.field0 else emptyList()
            else -> return emptyList()
        }
        val move = when (how) {
            is ControlMove.Typed -> "typed@${how.cursor}"
            is ControlMove.Set -> "set"
            is ControlMove.Pressed -> "pressed"
        }
        return listOf(ShellEvent.Key("$what $move $value"))
    }

    private fun controller(vararg controls: ControlShown, id: String = "form", focus: Int = 0, cancel: List<ShellEvent> = listOf(ShellEvent.Key("cancelled"))) =
        DialogController(
            DialogShown(id, "Form", controls.toList(), focus.toUInt(), cancel),
            send = { sent += (it as ShellEvent.Key).field0 },
            shell = { shelled += it.key },
            events = ::events,
        )

    @Test fun tabGoesRoundTheControlsAndTakesTheLineLeft() {
        val form = controller(line("title", "Buy milk", ControlSends.TaskField("title")), check("flag"), button("save", default = true))
        form.type("Buy bread")
        assertEquals(listOf("field title typed@9 Buy bread"), sent)
        sent.clear()
        assertEquals("flag", form.next()?.key)
        assertEquals("a line left is taken as it is", listOf("field title set Buy bread"), sent)
        assertEquals("save", form.next()?.key)
        assertEquals("title", form.next()?.key)
        assertEquals("save", form.previous()?.key)
    }

    @Test fun aLineLeftUnchangedSendsNothing() {
        val form = controller(line("title", "Buy milk", ControlSends.TaskField("title")), button("save"))
        form.next()
        assertTrue(sent.isEmpty())
    }

    @Test fun whatIsTypedIsHeldUntilTheCoreHasItAndTheCoreChangingItUnderneathWins() {
        val title = line("line", "bu", ControlSends.QuickAdd)
        val form = controller(title)
        form.type("buy")
        assertEquals("buy", form.text(form.control!!))
        form.update(dialog(title.copy(value = "buy")))
        assertEquals("buy", form.text(form.control!!))
        form.type("buy m")
        // A completion chosen: the core's line is another than the one typing began from.
        form.update(dialog(title.copy(value = "buy milk ")))
        assertEquals("buy milk ", form.text(form.control!!))
    }

    @Test fun aChoiceIsSentAsItStepsAndStopsAtItsEnds() {
        val form = controller(choice("clock", ControlSends.Setting("clock"), selected = 0, "12-hour", "24-hour"))
        assertEquals(1, form.step(1))
        assertEquals(listOf("setting clock set 24-hour"), sent)
        assertNull("no option past the last", form.step(1))
        assertEquals("a quick second press goes on from the first", 0, form.step(-1))
    }

    @Test fun aListMovesWithoutAnsweringAndItsButtonAnswersWithTheOneInHand() {
        val choices = choice("choices", ControlSends.Pick, selected = 0, "Inbox", "Work").copy(kind = ControlType.LIST)
        val form = controller(choices, button("yes", ControlSends.Pick, default = true), button("cancel", ControlSends.Press(listOf(ShellEvent.Key("cancelled")))))
        form.step(1)
        assertTrue("moving through a list answers nothing", sent.isEmpty())
        form.next()
        form.press()
        assertEquals(listOf("pick pressed Work"), sent)
    }

    @Test fun aChoiceIsFoundByTheFirstLetterOfItsOptionsRoundTheList() {
        val form = controller(choice("project", ControlSends.TaskField("project"), selected = 1, "Home", "Inbox", "Hobbies"))
        assertEquals(2, form.find("h"))
        assertEquals(0, form.find("H"))
        assertNull(form.find("z"))
    }

    @Test fun aCheckTogglesAndATogglePendingGoesOnFromItself() {
        val form = controller(check("flag", checked = false))
        assertEquals(true, form.toggle())
        assertEquals(false, form.toggle())
        assertEquals(listOf("field flag set true", "field flag set false"), sent)
    }

    @Test fun enterInALineThatSubmitsPressesItWithItsTextAndElsewhereThePressesTheDefault() {
        val quick = controller(line("line", "", ControlSends.QuickAdd), button("add", ControlSends.QuickAdd, default = true))
        quick.type("milk")
        sent.clear()
        quick.press()
        assertEquals(listOf("quick-add pressed milk"), sent)

        sent.clear()
        val task = controller(line("title", "a", ControlSends.TaskField("title")), button("save", ControlSends.Press(listOf(ShellEvent.Key("saved"))), default = true))
        task.type("ab")
        sent.clear()
        task.press()
        assertEquals("the line is taken, then the default pressed", listOf("field title set ab", "saved"), sent)
    }

    @Test fun aQuestionsButtonAnswersWithWhatItsLineHolds() {
        val form = controller(line("line", "", ControlSends.Answer), button("yes", ControlSends.Answer, default = true))
        form.type("tomorrow")
        assertTrue("typing in a question's line sends nothing", sent.isEmpty())
        form.next()
        form.press()
        assertEquals(listOf("answer pressed tomorrow"), sent)
    }

    @Test fun cancelSendsWhatTheDialogSaysAndAShellButtonIsTheDevicesOwn() {
        val form = controller(button("import", ControlSends.Shell("import")))
        form.press()
        assertEquals(listOf("import"), shelled)
        assertTrue(form.cancel())
        assertEquals(listOf("cancelled"), sent)
        assertFalse("a settings page has nothing to cancel", controller(cancel = emptyList()).cancel())
    }

    @Test fun aButtonsLetterPressesIt() {
        val form = controller(line("title", "", ControlSends.TaskField("title")), button("sync-now", ControlSends.Press(listOf(ShellEvent.Key("synced"))), letter = "n"))
        assertTrue(form.pressLetter("n"))
        assertFalse(form.pressLetter("q"))
        assertEquals(listOf("synced"), sent)
    }

    @Test fun anotherDialogStartsAfreshOnItsFocusAndTheSameOneKeepsTheControlInHand() {
        val title = line("title", "", ControlSends.TaskField("title"))
        val save = button("save")
        val form = controller(title, save)
        form.next()
        assertTrue(form.update(dialog(ControlShown_text("refusal", "Too long"), title, save)))
        assertEquals("the control in hand kept by its key", "save", form.control?.key)
        assertFalse(form.update(DialogShown("other", "Other", listOf(title, save), 0u, emptyList())))
        assertEquals("title", form.control?.key)
    }

    // ---- building the core's records by hand ----

    private fun dialog(vararg controls: ControlShown) = DialogShown("form", "Form", controls.toList(), 0u, listOf(ShellEvent.Key("cancelled")))

    private fun control(key: String, kind: ControlType, sends: ControlSends, value: String = "") = ControlShown(
        key = key, kind = kind, label = key, value = value, hint = "", example = "", options = emptyList(), selected = null,
        checked = false, `field` = null, syntax = null, letter = "", asks = false, default = false, cancel = false, sends = sends,
    )

    private fun line(key: String, value: String, sends: ControlSends) = control(key, ControlType.LINE, sends, value)

    private fun check(key: String, checked: Boolean = false) = control(key, ControlType.CHECK, ControlSends.TaskField(key)).copy(checked = checked)

    private fun button(key: String, sends: ControlSends = ControlSends.Nothing, default: Boolean = false, letter: String = "") =
        control(key, ControlType.BUTTON, sends).copy(default = default, letter = letter)

    private fun choice(key: String, sends: ControlSends, selected: Int, vararg titles: String) =
        control(key, ControlType.CHOICE, sends).copy(options = titles.map(::option), selected = selected.toUInt())

    private fun option(title: String) = Choice(title, title, 0u, null, null, null, null, null, null)

    @Suppress("FunctionName")
    private fun ControlShown_text(key: String, text: String) = control(key, ControlType.TEXT, ControlSends.Nothing, text)
}
