package dev.emassey0135.lumenna

import android.view.KeyEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class KeysOfTest {
    @Test
    fun aKeyAsTheSharedTableWritesItIsAndroidsKeyAndModifiers() {
        assertEquals(Keys(KeyEvent.KEYCODE_N, ctrl = true, shift = true), keysOf("Ctrl+Shift+N"))
        assertEquals(Keys(KeyEvent.KEYCODE_F10, shift = true), keysOf("Shift+F10"))
        assertEquals(Keys(KeyEvent.KEYCODE_PAGE_DOWN, ctrl = true), keysOf("Ctrl+Page Down"))
        assertEquals(Keys(KeyEvent.KEYCODE_COMMA, ctrl = true), keysOf("Ctrl+Comma"))
        assertEquals(Keys(KeyEvent.KEYCODE_5, ctrl = true), keysOf("Ctrl+5"))
        assertEquals(Keys(KeyEvent.KEYCODE_F6), keysOf("F6"))
    }

    @Test
    fun aPcKeyboardsDeleteIsForwardDeleteNotBackspace() {
        assertEquals(Keys(KeyEvent.KEYCODE_FORWARD_DEL), keysOf("Delete"))
        assertEquals(Keys(KeyEvent.KEYCODE_MENU), keysOf("Applications key"))
    }

    @Test
    fun aKeyAndroidHasNoNameForIsLeftOut() {
        assertNull(keysOf("Ctrl+Pause"))
        assertNull(keysOf("F13"))
    }
}
