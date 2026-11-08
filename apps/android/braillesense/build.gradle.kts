import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import javax.inject.Inject

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
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

// The BrailleSense 7 app. Its package is the one Android ID that is not the workspace's
// `package_id`: the device's launcher lists `com.extra.han.*` in its Programs menu, and its screen
// reader yields the braille keys to such a package, by its name alone (`records`' metadata test).
android {
    namespace = "com.extra.han.lumenna"
    compileSdk = 37

    defaultConfig {
        applicationId = "com.extra.han.lumenna"
        // Android 15 is every BrailleSense 7; the screens use nothing newer than the shared core.
        minSdk = 28
        targetSdk = 37
        versionCode = rootProject.extra["lumennaVersionCode"] as Int
        versionName = rootProject.extra["lumennaVersion"] as String
        resValue("string", "app_name", rootProject.extra["lumennaName"] as String)
        ndk { abiFilters += listOf("arm64-v8a") }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }

    buildFeatures {
        aidl = true
        buildConfig = true
        compose = true
        resValues = true
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
    implementation(project(":shared"))
    // `Core`'s Compose state and the activity it runs on.
    val bom = platform(libs.compose.bom)
    implementation(bom)
    implementation(libs.compose.ui)
    implementation(libs.activity.compose)
    implementation(libs.work.runtime)
    implementation(libs.androidx.core)
    testImplementation(libs.junit)
}
