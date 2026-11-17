package dev.emassey0135.orbit

import java.io.BufferedReader
import java.io.InputStreamReader
import java.nio.charset.StandardCharsets

/**
 * What Orbit keeps where only its own apps may read it, read through root as Orbit's own SDK reads
 * it: its settings provider wants Orbit's signature, and the screen reader's settings are in its
 * own data. Magisk ships on every Speak and Player, so `su` is there; the person may still refuse
 * it, which is a [RootFailure] for the app to report, never a default taken in silence.
 *
 * `su` started from an app keeps the app's mount namespace, in which /data holds only the app's
 * own data, so another app's files are read through init's root, `/proc/1/root`. Ported from the
 * Satellite launcher (`RootQuery`, Apache-2.0, the same author's).
 */
public object Root {
    /** Runs [command] as root, off the main thread: what it printed, or why it could not. */
    public fun run(command: String): Result<String> {
        var process: Process? = null
        return try {
            process = ProcessBuilder("su", "-c", command).redirectErrorStream(true).start()
            val output = BufferedReader(InputStreamReader(process.inputStream, StandardCharsets.UTF_8)).use { it.readText() }
            val status = process.waitFor()
            if (status == 0) Result.success(output) else Result.failure(RootFailure(output.trim().ifEmpty { "su exited with $status" }))
        } catch (error: Exception) {
            Result.failure(RootFailure(error.message ?: error.javaClass.simpleName))
        } finally {
            process?.destroy()
        }
    }

    /** Another app's file, by its path under /data, as root sees it. */
    public fun dataPath(path: String): String = "/proc/1/root$path"

    /** The `value=` field of a single-row content query from [from] on; null if there is none. */
    public fun valueField(output: String, from: Int): String? {
        val at = output.indexOf("value=", from)
        if (at < 0) return null
        var end = output.indexOf(", type=", at)
        if (end < 0) end = output.indexOf('\n', at)
        if (end < 0) end = output.length
        return output.substring(at + 6, end).trim()
    }
}

/** Root was refused, or what was asked of it failed: [reason] as `su` or the command said it. */
public class RootFailure(public val reason: String) : Exception(reason)
