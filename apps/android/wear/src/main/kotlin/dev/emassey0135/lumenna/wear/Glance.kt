package dev.emassey0135.lumenna.wear

import android.app.PendingIntent
import android.content.ComponentName
import android.content.Context
import androidx.wear.protolayout.ActionBuilders
import androidx.wear.protolayout.LayoutElementBuilders.LayoutElement
import androidx.wear.protolayout.TypeBuilders.StringLayoutConstraint
import androidx.wear.protolayout.expression.DynamicBuilders.DynamicInstant
import androidx.wear.protolayout.expression.DynamicBuilders.DynamicInt32
import androidx.wear.protolayout.expression.DynamicBuilders.DynamicString
import androidx.wear.protolayout.layout.column
import androidx.wear.protolayout.material3.MaterialScope
import androidx.wear.protolayout.material3.Typography
import androidx.wear.protolayout.material3.primaryLayout
import androidx.wear.protolayout.material3.text
import androidx.wear.protolayout.modifiers.LayoutModifier
import androidx.wear.protolayout.modifiers.clearSemantics
import androidx.wear.protolayout.modifiers.clickable
import androidx.wear.protolayout.modifiers.contentDescription
import androidx.wear.protolayout.types.LayoutString
import androidx.wear.protolayout.types.layoutString
import androidx.wear.tiles.Material3TileService
import androidx.wear.tiles.RequestBuilders.TileRequest
import androidx.wear.tiles.TileBuilders.Tile
import androidx.wear.tiles.TileService
import androidx.wear.tiles.timeInterval
import androidx.wear.tiles.timeline
import androidx.wear.tiles.timelineEntry
import androidx.wear.tiles.tile
import androidx.wear.watchface.complications.data.ComplicationData
import androidx.wear.watchface.complications.data.ComplicationType
import androidx.wear.watchface.complications.data.NoDataComplicationData
import androidx.wear.watchface.complications.data.PlainComplicationText
import androidx.wear.watchface.complications.data.RangedValueComplicationData
import androidx.wear.watchface.complications.data.ShortTextComplicationData
import androidx.wear.watchface.complications.datasource.ComplicationDataSourceUpdateRequester
import androidx.wear.watchface.complications.datasource.ComplicationDataTimeline
import androidx.wear.watchface.complications.datasource.ComplicationRequest
import androidx.wear.watchface.complications.datasource.SuspendingTimelineComplicationDataSourceService
import androidx.wear.watchface.complications.datasource.TimeInterval
import androidx.wear.watchface.complications.datasource.TimelineEntry
import dev.emassey0135.lumenna.CoreHolder
import dev.emassey0135.lumenna.Glances
import dev.emassey0135.lumenna.Launching
import dev.emassey0135.lumenna.app.GlanceEntry
import java.time.Duration
import java.time.Instant
import kotlin.time.toKotlinDuration
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

// The watch's tile and its complication for watch faces, drawn from the core's glance
// (`Glances`): what it says and in its order, and nothing decided here. Each takes the glance's
// timeline whole, an entry for each moment, so the system switches between them by itself.

/** The glance from now, read off the main thread. */
private suspend fun entries(context: Context): List<GlanceEntry> = withContext(Dispatchers.IO) {
    val core = (context.applicationContext as? CoreHolder)?.core?.getOrNull() ?: return@withContext emptyList()
    runCatching { Glances.entries(context, core) }.getOrDefault(emptyList())
}

/** When each entry stops being true: when the next starts, the last a day on. */
private fun until(entries: List<GlanceEntry>, index: Int, start: Instant): Instant =
    entries.getOrNull(index + 1)?.let(Glances::at)?.takeIf { it.isAfter(start) } ?: start.plus(Duration.ofDays(1))

/**
 * Today on a tile: what is happening now, the next block, and a running timer counted by the
 * watch itself from the core's `countingFrom`. TalkBack hears the glance's `spoken` sentence;
 * tapped, it opens the app on Today.
 */
class LumennaTile : Material3TileService() {
    override suspend fun MaterialScope.tileResponse(requestParams: TileRequest): Tile {
        val entries = entries(this@LumennaTile)
        val open = clickable(
            ActionBuilders.launchAction(
                ComponentName(this@LumennaTile, MainActivity::class.java),
                mapOf(Launching.ID to ActionBuilders.stringExtra(Launching.TODAY)),
            ),
        )
        if (entries.isEmpty()) return tile(timeline(timelineEntry(primaryLayout(mainSlot = { text("Lumenna".layoutString) }, onClick = open))))
        val now = Instant.now()
        val layouts = entries.mapIndexed { index, entry ->
            val start = Glances.at(entry) ?: now
            val end = until(entries, index, start)
            timelineEntry(
                layout(entry, open),
                timeInterval(Duration.between(Instant.EPOCH, start).toKotlinDuration(), Duration.between(Instant.EPOCH, end).toKotlinDuration()),
            )
        }
        return tile(timeline(*layouts.toTypedArray()))
    }

    private fun MaterialScope.layout(entry: GlanceEntry, open: androidx.wear.protolayout.ModifiersBuilders.Clickable): LayoutElement =
        primaryLayout(
            titleSlot = { text(entry.headline.title.layoutString, modifier = LayoutModifier.clearSemantics()) },
            mainSlot = {
                val lines = buildList {
                    if (entry.headline.detail.isNotEmpty()) add(entry.headline.detail)
                    entry.next?.let { add(it.text) }
                }
                val parts = lines.map { text(it.layoutString, typography = Typography.BODY_MEDIUM, maxLines = 2) }.toMutableList()
                entry.moment.timer?.let { timer -> Glances.instant(timer.countingFrom)?.let { parts.add(timerText(it)) } }
                column(*parts.toTypedArray(), modifier = LayoutModifier.contentDescription(entry.spoken))
            },
            onClick = open,
        )

    /** The time since [since], counted by the watch: hours, minutes and seconds. */
    private fun MaterialScope.timerText(since: Instant): LayoutElement {
        val elapsed = DynamicInstant.withSecondsPrecision(since).durationUntil(DynamicInstant.platformTimeWithSecondsPrecision())
        val two = DynamicInt32.IntFormatter.Builder().setMinIntegerDigits(2).build()
        val shown: DynamicString = elapsed.toIntHours().format()
            .concat(DynamicString.constant(":")).concat(elapsed.minutesPart.format(two))
            .concat(DynamicString.constant(":")).concat(elapsed.secondsPart.format(two))
        val seconds = Duration.between(since, Instant.now()).seconds.coerceAtLeast(0)
        val static = "%d:%02d:%02d".format(seconds / 3600, seconds / 60 % 60, seconds % 60)
        return text(
            LayoutString(static, shown, StringLayoutConstraint.Builder("00:00:00").build()),
            typography = Typography.NUMERAL_MEDIUM,
        )
    }

    companion object {
        /** Asks the system to draw the tile again. */
        fun refresh(context: Context) = TileService.getUpdater(context).requestUpdate(LumennaTile::class.java)
    }
}

/**
 * What is happening now, for a watch face: as short text, the glance's headline; as a ranged
 * value, how far through the block under way it is, from its start and end, counted by the
 * watch itself where it can (Wear OS 4 and later). Its description is the glance's `spoken`.
 */
class LumennaComplication : SuspendingTimelineComplicationDataSourceService() {
    override suspend fun onComplicationRequest(request: ComplicationRequest): ComplicationDataTimeline? {
        val entries = entries(this)
        if (entries.isEmpty()) return null
        val now = Instant.now()
        val data = entries.mapIndexed { index, entry ->
            val start = Glances.at(entry)?.takeIf { it.isAfter(now) } ?: now
            TimeInterval(start, until(entries, index, start)) to data(entry, request.complicationType, start)
        }
        return ComplicationDataTimeline(data.first().second, data.drop(1).map { (validity, shown) -> TimelineEntry(validity, shown) })
    }

    override fun getPreviewData(type: ComplicationType): ComplicationData? = null

    private fun tap() = PendingIntent.getActivity(
        this, 0, Launching.intent(this, MainActivity::class.java, Launching.TODAY),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    /** What [entry] shows from [from] on. */
    private fun data(entry: GlanceEntry, type: ComplicationType, from: Instant): ComplicationData {
        val said = PlainComplicationText.Builder(entry.spoken).build()
        if (type == ComplicationType.RANGED_VALUE) {
            val block = entry.moment.current ?: return NoDataComplicationData()
            val start = Glances.instant(block.startsAt) ?: return NoDataComplicationData()
            val end = Glances.instant(block.endsAt) ?: return NoDataComplicationData()
            val length = Duration.between(start, end).seconds.toFloat().coerceAtLeast(1f)
            val gone = Duration.between(start, from).seconds.toFloat().coerceIn(0f, length)
            val builder = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
                val passed = DynamicInstant.withSecondsPrecision(start)
                    .durationUntil(DynamicInstant.platformTimeWithSecondsPrecision())
                    .toIntSeconds().asFloat()
                RangedValueComplicationData.Builder(passed, gone, 0f, length, said)
            } else {
                RangedValueComplicationData.Builder(gone, 0f, length, said)
            }
            return builder.setText(PlainComplicationText.Builder(block.title).build()).setTapAction(tap()).build()
        }
        return ShortTextComplicationData.Builder(PlainComplicationText.Builder(entry.headline.title).build(), said)
            .setTapAction(tap())
            .build()
    }

    companion object {
        /** Asks every watch face showing it to take it again. */
        fun refresh(context: Context) =
            ComplicationDataSourceUpdateRequester.create(context, ComponentName(context, LumennaComplication::class.java)).requestUpdateAll()
    }
}
