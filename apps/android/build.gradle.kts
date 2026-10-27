plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.android.library) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.ksp) apply false
}

// What Lumenna is, from the workspace's one source, `[workspace.metadata.lumenna]` in the
// checkout's Cargo.toml, so a version bump is one edit there and every platform's build says it.
// Its version code counts up with the version: 1.2.3 is 1002003.
val lumenna: Map<String, String> = run {
    var section = ""
    rootDir.resolve("../../Cargo.toml").readLines().mapNotNull { whole ->
        val line = whole.trim()
        when {
            line.startsWith("[") -> { section = line.trim('[', ']'); null }
            section == "workspace.metadata.lumenna" && "=" in line ->
                line.substringBefore("=").trim() to line.substringAfter("=").trim().trim('"')
            else -> null
        }
    }.toMap()
}
val lumennaVersion = lumenna.getValue("version")
extra["lumennaName"] = lumenna.getValue("name")
extra["lumennaVersion"] = lumennaVersion
extra["lumennaVersionCode"] = lumennaVersion.split('.').map { it.takeWhile(Char::isDigit).toInt() }
    .let { (major, minor, patch) -> major * 1_000_000 + minor * 1_000 + patch }
