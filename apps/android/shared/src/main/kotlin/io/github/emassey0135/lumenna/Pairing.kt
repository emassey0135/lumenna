package io.github.emassey0135.lumenna

import android.content.Context
import android.os.Build

/** This device's name, as the other devices will list it until someone renames it. */
fun deviceName(context: Context): String =
    android.provider.Settings.Global.getString(context.contentResolver, android.provider.Settings.Global.DEVICE_NAME)
        ?: Build.MODEL
