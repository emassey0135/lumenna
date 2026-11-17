package dev.emassey0135.braillesense.edit

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context

/** Android's clipboard, which the device's own boxes copy to and paste from, as plain text labelled [label]. */
public class SystemClipboard(private val context: Context, private val label: CharSequence) : Clipboard {
    private val manager: ClipboardManager? get() = context.getSystemService(ClipboardManager::class.java)

    override fun text(): String? = runCatching {
        manager?.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()
    }.getOrNull()

    override fun set(text: String?) {
        val manager = manager ?: return
        runCatching { if (text == null) manager.clearPrimaryClip() else manager.setPrimaryClip(ClipData.newPlainText(label, text)) }
    }
}
