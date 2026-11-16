package dev.emassey0135.orbit

import dev.emassey0135.orbit.OrbitMenu.Item
import dev.emassey0135.orbit.OrbitMenu.Outcome
import org.junit.Assert.assertEquals
import org.junit.Test

class OrbitMenuTest {
    private fun menu() = OrbitMenu(
        listOf(
            Item("File", 'f', listOf(Item("New", 'n', value = "new"), Item("Open", 'o', value = "open"), Item("Save", 's', value = "save"))),
            Item("Edit", 'e', listOf(Item("Copy", 'y', value = "copy"), Item("Cut", 'c', value = "cut"), Item("Clear", 'c', value = "clear"))),
            Item("Exit", 'x', value = "exit"),
        ),
    )

    @Test
    fun up_and_down_go_round_a_level() {
        val menu = menu()
        menu.up()
        assertEquals("Exit", menu.current?.name)
        menu.down()
        assertEquals("File", menu.current?.name)
    }

    @Test
    fun right_opens_a_submenu_on_its_first_item_and_left_goes_back_onto_it() {
        val menu = menu()
        menu.down()
        assertEquals(Outcome.Opened, menu.right())
        assertEquals("Copy" to 1, menu.current?.name to menu.depth)
        assertEquals(Outcome.Back, menu.left())
        assertEquals("Edit" to 0, menu.current?.name to menu.depth)
        assertEquals(Outcome.Closed, menu.left())
    }

    @Test
    fun right_on_an_item_does_nothing_and_select_chooses_it() {
        val menu = menu()
        menu.last()
        assertEquals(Outcome.Moved, menu.right())
        assertEquals(Outcome.Chosen("exit"), menu.select())
    }

    @Test
    fun a_letter_selects_its_only_item_and_goes_round_several() {
        val menu = menu()
        assertEquals("the submenu opens", Outcome.Opened, menu.accessKey('e'))
        assertEquals(Outcome.Moved, menu.accessKey('c'))
        assertEquals("Cut", menu.current?.name)
        menu.accessKey('c')
        assertEquals("Clear", menu.current?.name)
        menu.accessKey('c')
        assertEquals("Cut", menu.current?.name)
        assertEquals(Outcome.Chosen("copy"), menu.accessKey('y'))
        assertEquals(Outcome.Refused, menu.accessKey('q'))
    }
}
