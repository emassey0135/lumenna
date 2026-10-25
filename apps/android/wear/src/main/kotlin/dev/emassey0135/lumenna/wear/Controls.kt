package dev.emassey0135.lumenna.wear

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.graphics.drawable.Icon
import androidx.wear.watchface.complications.data.ComplicationData
import androidx.wear.watchface.complications.data.ComplicationType
import androidx.wear.watchface.complications.data.MonochromaticImage
import androidx.wear.watchface.complications.data.MonochromaticImageComplicationData
import androidx.wear.watchface.complications.data.PlainComplicationText
import androidx.wear.watchface.complications.data.ShortTextComplicationData
import androidx.wear.watchface.complications.data.SmallImage
import androidx.wear.watchface.complications.data.SmallImageComplicationData
import androidx.wear.watchface.complications.data.SmallImageType
import androidx.wear.watchface.complications.datasource.ComplicationRequest
import androidx.wear.watchface.complications.datasource.SuspendingComplicationDataSourceService
import dev.emassey0135.lumenna.Launching
import dev.emassey0135.lumenna.app.Control
import dev.emassey0135.lumenna.app.controls

// The core's controls on the watch (`controls()`): New Task, its launch shortcut, as a
// complication and as the tile's button. Each opens the app on `new-task`, which the app does as
// it does from anywhere: quick add, the system's input screen straight away.

/** The core's New Task control, in Android's sentence case. */
fun newTaskControl(): Control = controls().first { it.id == NewTaskComplication.NEW_TASK }

/** What opens the watch app on quick add. */
fun newTaskIntent(context: Context): Intent = Launching.intent(context, MainActivity::class.java, NewTaskComplication.NEW_TASK)

/**
 * New Task for a watch face: an icon, alone or with the core's words, named by them for
 * TalkBack. Tapped, it opens quick add. It never changes, so it is never asked for again.
 */
class NewTaskComplication : SuspendingComplicationDataSourceService() {
    override suspend fun onComplicationRequest(request: ComplicationRequest): ComplicationData? =
        data(this, request.complicationType)

    override fun getPreviewData(type: ComplicationType): ComplicationData? = data(this, type)

    companion object {
        /** New Task's launch `id`. */
        const val NEW_TASK = "new-task"

        /**
         * Its own request code: an intent differing only in its extras is the same pending intent,
         * and the Now complication's (code 0) would otherwise open on New Task, or this on Today.
         */
        private const val REQUEST = 1

        /** What a tap does: opens the app on New Task. */
        fun tap(context: Context): PendingIntent = PendingIntent.getActivity(
            context, REQUEST, newTaskIntent(context), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        /** What it shows as [type], or nothing for a type it does not offer. */
        fun data(context: Context, type: ComplicationType): ComplicationData? {
            val words = newTaskControl().sentence
            val said = PlainComplicationText.Builder(words).build()
            val icon = Icon.createWithResource(context, R.drawable.ic_new_task)
            val monochrome = MonochromaticImage.Builder(icon).build()
            val tap = tap(context)
            return when (type) {
                ComplicationType.SHORT_TEXT ->
                    ShortTextComplicationData.Builder(said, said).setMonochromaticImage(monochrome).setTapAction(tap).build()
                ComplicationType.MONOCHROMATIC_IMAGE ->
                    MonochromaticImageComplicationData.Builder(monochrome, said).setTapAction(tap).build()
                ComplicationType.SMALL_IMAGE ->
                    SmallImageComplicationData.Builder(SmallImage.Builder(icon, SmallImageType.ICON).build(), said).setTapAction(tap).build()
                else -> null
            }
        }
    }
}
