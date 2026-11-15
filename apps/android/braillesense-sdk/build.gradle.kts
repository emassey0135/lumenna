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
}

/**
 * Builds liblouis for the device and gathers its tables (`build-louis.sh`): what is built
 * already is left, so this costs nothing when nothing changed.
 */
abstract class BuildLouis @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:OutputDirectory abstract val jniLibs: DirectoryProperty
    @get:OutputDirectory abstract val assets: DirectoryProperty
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
                assets.get().asFile.absolutePath,
                profile.get(),
            )
        }
    }
}

// An unofficial SDK for the HIMS BrailleSense 7, found by reading the device and its built-in apps:
// its braille keys and QWERTY keyboard, speech and cells through its own services (the AIDL, under
// the vendor's descriptors), the person's Global Options, liblouis in their table, panning, how the
// device reads a control, and an edit box that edits and speaks as the device's own. It depends on
// nothing of Lumenna's, so that it can go to a repository of its own and other apps can use it.
android {
    namespace = "dev.emassey0135.braillesense"
    compileSdk = 37

    defaultConfig {
        // Android 15 is every BrailleSense 7; nothing here needs more than the apps it serves.
        minSdk = 28
        ndk { abiFilters += listOf("arm64-v8a") }
        consumerProguardFiles("consumer-rules.pro")
    }

    buildFeatures {
        aidl = true
    }

    testOptions {
        // Only the log touches Android in what the tests reach, which then does nothing.
        unitTests.isReturnDefaultValues = true
    }
}

androidComponents {
    onVariants { variant ->
        val name = variant.name.replaceFirstChar { it.uppercase() }
        val louis = tasks.register<BuildLouis>("buildLouis$name") {
            script.set(file("build-louis.sh").absolutePath)
            profile.set(if (variant.buildType == "release") "release" else "debug")
            jniLibs.set(layout.buildDirectory.dir("generated/louis/${variant.name}/jniLibs"))
            assets.set(layout.buildDirectory.dir("generated/louis/${variant.name}/assets"))
        }
        variant.sources.jniLibs?.addGeneratedSourceDirectory(louis, BuildLouis::jniLibs)
        variant.sources.assets?.addGeneratedSourceDirectory(louis, BuildLouis::assets)
    }
}

dependencies {
    testImplementation(libs.junit)
}
