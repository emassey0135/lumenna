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
 * Builds the Rust core for the phone and generates its Kotlin bindings (`build-core.sh`),
 * as Xcode's pre-build phase does for the Apple apps. Both land in the build directory as
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
    namespace = "io.github.emassey0135.lumenna"
    compileSdk = 37

    defaultConfig {
        applicationId = "io.github.emassey0135.lumenna"
        minSdk = 28
        targetSdk = 37
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    buildFeatures {
        compose = true
    }

    packaging {
        // JNA's own native libraries are found by name at run time.
        jniLibs.useLegacyPackaging = true
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
    val bom = platform(libs.compose.bom)
    implementation(bom)
    implementation(libs.compose.ui)
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.activity.compose)
    // UniFFI's Kotlin bindings call the core through JNA.
    implementation("${libs.jna.get()}@aar")

    androidTestImplementation(bom)
    androidTestImplementation(libs.compose.ui.test.junit4)
    androidTestImplementation(libs.compose.ui.test.junit4.accessibility)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.junit)
    // Compose's test library brings Espresso 3.5, which calls an InputManager method Android
    // 17 no longer has; every test failed before reaching the app.
    androidTestImplementation(libs.espresso.core)
    debugImplementation(libs.compose.ui.test.manifest)
}
