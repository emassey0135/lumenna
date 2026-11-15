package dev.emassey0135.braillesense

import dev.emassey0135.braillesense.reading.BrailleSigns
import dev.emassey0135.braillesense.reading.ControlRole
import dev.emassey0135.braillesense.reading.DeviceLine
import dev.emassey0135.braillesense.reading.DeviceReading
import dev.emassey0135.braillesense.reading.LineSettings
import dev.emassey0135.braillesense.reading.RolePlace
import dev.emassey0135.braillesense.reading.deviceReading
import dev.emassey0135.braillesense.reading.hotKeyDots
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ReadingTest {
    private fun settings(voice: RolePlace = RolePlace.AFTER, braille: RolePlace = RolePlace.BEFORE) =
        LineSettings(voice, braille, voiceLetter = true, brailleLetter = true, voiceHotKey = false, voiceHotKeyDots = false, brailleHotKey = false, index = true)

    private val signs = BrailleSigns("\"<", "\">", "\"\"\"", " 3o")

    private fun read(line: DeviceLine, settings: LineSettings = settings()) = deviceReading(line, settings, signs)

    private fun shown(reading: DeviceReading) = reading.before.joinToString("") { it.text } + reading.body + reading.after.joinToString("") { it.text }

    @Test fun aMenuItemNamesItsRoleWhereEachSettingPutsIt() {
        val reading = read(DeviceLine(ControlRole.MENU_ITEM, speech = "New", letter = "n"))
        assertEquals("New (n) menu item", reading.speech)
        assertEquals("mi New \"<n\">", shown(reading))
        assertTrue(reading.before[0].computer)
    }

    @Test fun anItemThatOpensADialogSaysSoBeforeItsRoleAndEndsInTheDevicesSign() {
        val reading = read(DeviceLine(ControlRole.MENU_ITEM, speech = "Rename", letter = "r", asks = true))
        assertEquals("Rename (r) dialog menu item", reading.speech)
        assertEquals("mi Rename \"<r\"> \"\"\"", shown(reading))
    }

    @Test fun aSubmenuIsAPulldownAndOnlyASubmenuIs() {
        val reading = read(DeviceLine(ControlRole.MENU, speech = "File", letter = "f"))
        assertEquals("File (f) pulldown Menu", reading.speech)
        assertEquals("mn File \"<f\"> 3o", shown(reading))
        assertFalse(read(DeviceLine(ControlRole.MENU_ITEM, speech = "New")).speech.contains("pulldown"))
    }

    @Test fun anEditBoxShowsItsLabelAheadOfItsText() {
        val reading = read(DeviceLine(ControlRole.EDIT_BOX, label = "Title", speech = "Buy milk"))
        assertEquals("Title: Buy milk edit box", reading.speech)
        assertEquals("Buy milk", reading.body)
        assertEquals("eb Title: Buy milk", shown(reading))
    }

    @Test fun aComboBoxGivesItsPlaceOnlyWhenTheDeviceReportsIndexes() {
        val combo = DeviceLine(ControlRole.COMBO_BOX, label = "Priority", speech = "High", position = 2, count = 5)
        val reading = read(combo)
        assertEquals("Priority: High 2/5 combo box", reading.speech)
        assertEquals("cb Priority: High 2/5", shown(reading))
        assertEquals("Priority: High combo box", read(combo, settings().copy(index = false)).speech)
    }

    @Test fun rolesSetOffLeaveTheLineAloneAndAfterPutsThemLast() {
        val row = DeviceLine(ControlRole.LIST_ITEM, speech = "Today")
        assertEquals("Today", read(row, settings(RolePlace.OFF, RolePlace.OFF)).speech)
        assertEquals("Today", shown(read(row, settings(RolePlace.OFF, RolePlace.OFF))))
        assertEquals("list item Today", read(row, settings(RolePlace.BEFORE, RolePlace.AFTER)).speech)
        assertEquals("Today li", shown(read(row, settings(RolePlace.BEFORE, RolePlace.AFTER))))
    }

    @Test fun aRowsNumberComesLastWhenTheDeviceReportsIndexes() {
        val reading = read(DeviceLine(ControlRole.LIST_ITEM, speech = "Today", letter = "t", detail = "3 tasks", position = 3, count = 12))
        assertEquals("Today (t), 3 tasks 3/12 list item", reading.speech)
        assertEquals("li Today \"<t\"> 3 tasks 3/12", shown(reading))
    }

    @Test fun aHotKeyIsSaidAndShownAsTheDeviceDoes() {
        val shows = settings().copy(voiceHotKey = true, brailleHotKey = true)
        val item = DeviceLine(ControlRole.MENU_ITEM, speech = "New", asks = true, hotKey = "Enter-n")
        val reading = read(item, shows)
        assertEquals("New dialog Enter-n menu item", reading.speech)
        assertEquals("mi New ent-n \"\"\"", shown(reading))
        assertEquals("New dialog menu item", read(item, shows.copy(voiceHotKey = false)).speech)
    }

    @Test fun theDotPatternsSettingSaysAHotKeyAsDotsAndLeavesTheCellsItsLetter() {
        val shows = settings().copy(voiceHotKey = true, voiceHotKeyDots = true, brailleHotKey = true)
        val reading = read(DeviceLine(ControlRole.MENU_ITEM, speech = "New", hotKey = "Enter-n"), shows)
        assertEquals("New Enter-dots 1-3-4-5 menu item", reading.speech)
        assertEquals("mi New ent-n", shown(reading))
        assertEquals("Space-dot 1", hotKeyDots("Space-a"))
    }

    @Test fun aLetterComesAfterTheNameAndBeforeTheDetail() {
        val reading = read(DeviceLine(ControlRole.LIST_ITEM, speech = "Today", letter = "t", detail = "3 tasks"), settings(RolePlace.OFF, RolePlace.OFF))
        assertEquals("Today (t), 3 tasks", reading.speech)
        assertEquals("Today \"<t\"> 3 tasks", shown(reading))
    }
}
