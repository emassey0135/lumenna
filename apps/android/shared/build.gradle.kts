import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import javax.inject.Inject

plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.compose)
}

/**
 * Builds the Rust core for the phone and the watch and generates its Kotlin bindings
 * (`build-core.sh`), as Xcode's pre-build phase does for the Apple apps. Both land in the build directory as
 * generated sources: nothing generated is committed. Cargo decides what is out of date, so
 * this always runs and costs a second or two when nothing changed.
 */
abstract class BuildCore @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:OutputDirectory abstract val jniLibs: DirectoryProperty
    @get:OutputDirectory abstract val kotlin: DirectoryProperty
    @get:Input abstract val profile: Property<String>
    @get:Input abstract val script: Property<String>

    init {
        outputs.upToDateWhen { false }
    }

    @TaskAction
    fun build() {
        exec.exec {
            commandLine(
                script.get(),
                jniLibs.get().asFile.absolutePath,
                kotlin.get().asFile.absolutePath,
                profile.get(),
            )
        }
    }
}

android {
    namespace = "io.github.emassey0135.lumenna.shared"
    compileSdk = 37

    defaultConfig {
        minSdk = 28
    }

    buildFeatures {
        compose = true
    }
}

androidComponents {
    onVariants { variant ->
        val name = variant.name.replaceFirstChar { it.uppercase() }
        val core = tasks.register<BuildCore>("buildCore$name") {
            script.set(rootProject.file("build-core.sh").absolutePath)
            profile.set(if (variant.buildType == "release") "release" else "debug")
            jniLibs.set(layout.buildDirectory.dir("generated/core/${variant.name}/jniLibs"))
            kotlin.set(layout.buildDirectory.dir("generated/core/${variant.name}/kotlin"))
        }
        variant.sources.jniLibs?.addGeneratedSourceDirectory(core, BuildCore::jniLibs)
        variant.sources.kotlin?.addGeneratedSourceDirectory(core, BuildCore::kotlin)
    }
}

dependencies {
    // Folding remembers what is folded through Compose's runtime, which both apps' UIs share.
    val bom = platform(libs.compose.bom)
    implementation(bom)
    implementation(libs.compose.runtime.saveable)
    implementation(libs.work.runtime)
    // The watch's own link to its phone (WatchLink), preferred to Iroh while they are near.
    api(libs.play.wearable)
    // UniFFI's Kotlin bindings call the core through JNA; the apps reach the core's types too.
    api("${libs.jna.get()}@aar")
}
