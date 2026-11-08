package dev.emassey0135.lumenna.orbit

import org.junit.Assert.assertEquals
import org.junit.Test

class DescribeTest {
    private val list = Drawn("Tasks", 3, 1, "Pay rent", Opened.List, "Nothing here")

    @Test fun aListOpeningIsAFocusChangeOfAListBox() {
        assertEquals(listOf(ReaderCall.FocusList("Tasks", 3, 1, "Pay rent")), describe(true, Why.Entered, false, list))
    }

    @Test fun aMoveThePersonMadeIsAnItemChange() {
        assertEquals(listOf(ReaderCall.ItemChanged("Tasks", 3, 1, "Pay rent")), describe(false, Why.Moved, true, list))
    }

    @Test fun whenTheAppLandsSomewhereItIsAnItemChange() {
        assertEquals(listOf(ReaderCall.ItemChanged("Tasks", 3, 1, "Pay rent")), describe(false, Why.Landed, true, list))
    }

    @Test fun aListEmptiedByAChangeSaysWhatItSaysEmpty() {
        val empty = list.copy(count = 0, item = "", empty = "No open tasks.")
        assertEquals(listOf(ReaderCall.Say("No open tasks.")), describe(false, Why.Turn, true, empty))
    }

    @Test fun aTurnThatOnlyRedrewSaysNothing() {
        assertEquals(emptyList<ReaderCall>(), describe(false, Why.Turn, false, list))
    }

    @Test fun anEmptyListSaysSoOnceUnlessItsHeadingAlreadyDoes() {
        val empty = list.copy(count = 0, item = "", empty = "No tasks.")
        assertEquals(listOf(ReaderCall.FocusList("Tasks", 0, 1, ""), ReaderCall.Say("No tasks.")), describe(true, Why.Entered, false, empty))
        val said = empty.copy(title = "Tasks, No tasks.")
        assertEquals(listOf(ReaderCall.FocusList("Tasks, No tasks.", 0, 1, "")), describe(true, Why.Entered, false, said))
    }

    @Test fun aQuestionOpensAsADialog() {
        val question = list.copy(title = "Delete?", item = "Yes", opened = Opened.Dialog)
        assertEquals(
            listOf(ReaderCall.Dialog("Delete?", "Yes"), ReaderCall.ItemChanged("Delete?", 3, 1, "Yes")),
            describe(true, Why.Entered, false, question),
        )
    }

    @Test fun aRowsActionsOpenAsAContextMenuThenTheirList() {
        val menu = list.copy(title = "Pay rent", item = "Mark done", opened = Opened.ContextMenu)
        assertEquals(
            listOf(ReaderCall.ContextMenu("Pay rent"), ReaderCall.ItemChanged("Pay rent", 3, 1, "Mark done")),
            describe(true, Why.Entered, false, menu),
        )
    }
}
