package dev.emassey0135.orbit

/**
 * What a key does in a dialog, as Orbit's own dialogs have it (the Editor's Save As): Tab and
 * Shift-Tab move between the controls, each active as soon as it is reached; an edit box takes its
 * own keys ([EditBox]); Up and Down change a combo box or a list box and Space with L and dots
 * 4 5 6 go to its ends, a cell finding an option by its letter; Up, Down or Select toggle a check
 * box; Select or Dot 8 presses a button or picks the option in hand; Space with E or Z (Escape) and
 * Dot 7 off an edit box cancel; and Left and Right never leave it. The menu key and everything else
 * are the app's.
 *
 * The app tells [OrbitReader] of the dialog as it opens (`dialog`), then of each control as it is
 * reached, by its kind: an edit box through its [EditBox], a combo box or list box with `focusChoice`
 * ([OrbitReader.ROLE_COMBO_BOX], [OrbitReader.ROLE_LIST_BOX]) and `itemChanged` as it changes, a
 * check box with `focusCheck`, a button with `focusControl` ([OrbitReader.ROLE_BUTTON]), and a line
 * to read with `focusText`.
 */
object OrbitDialog {
    /** A dialog's control, as Orbit's reader names it. */
    enum class Control { EDIT, COMBO_BOX, LIST_BOX, CHECK_BOX, BUTTON, TEXT }

    /** What a key does to the dialog and the control in hand. */
    sealed interface Does {
        /** Tab: on to the next control. */
        data object Next : Does

        /** Shift-Tab: back to the one before. */
        data object Previous : Does

        /** Cancel the dialog. */
        data object Cancel : Does

        /** Press the button, or pick the option in hand. */
        data object Press : Does

        /** Turn the check box over. */
        data object Toggle : Does

        /** Move the choice [by] options. */
        data class Step(val by: Int) : Does

        /** The choice's first option. */
        data object First : Does

        /** The choice's last option. */
        data object Last : Does

        /** The next option starting with the letter of these [dots]. */
        data class Find(val dots: Int) : Does

        /** The edit box's own key: hand it to its [EditBox]. */
        data object Edit : Does

        /** A key the dialog takes and does nothing with, so nothing else does either. */
        data object Nothing : Does

        /** Not the dialog's: the menu key, and what the control does not know. */
        data object NotTaken : Does
    }

    /** What [key] does with [control] in hand. */
    fun does(key: Key, control: Control): Does {
        when (key) {
            Key.Tab -> return Does.Next
            Key.BackTab -> return Does.Previous
            Key.Escape -> return Does.Cancel
            Key.Menu -> return Does.NotTaken
            else -> {}
        }
        if (control == Control.EDIT) return Does.Edit
        return when (key) {
            Key.Dot7 -> Does.Cancel
            Key.Left, Key.Right -> Does.Nothing
            else -> when (control) {
                Control.COMBO_BOX, Control.LIST_BOX -> when (key) {
                    Key.Up -> Does.Step(-1)
                    Key.Down -> Does.Step(1)
                    Key.First -> Does.First
                    Key.Last -> Does.Last
                    is Key.Cell -> Does.Find(key.dots)
                    Key.Select, Key.Dot8 -> Does.Press
                    else -> Does.NotTaken
                }
                Control.CHECK_BOX -> when (key) {
                    Key.Up, Key.Down, Key.Select, Key.Dot8 -> Does.Toggle
                    else -> Does.NotTaken
                }
                Control.BUTTON -> if (key == Key.Select || key == Key.Dot8) Does.Press else Does.NotTaken
                Control.TEXT -> if (key == Key.Select || key == Key.Dot8) Does.Nothing else Does.NotTaken
                Control.EDIT -> Does.Edit
            }
        }
    }
}
