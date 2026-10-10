plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

// The Wear OS app: Compose for Wear OS over the same core and shared code as the phone
// (`:shared`). A standalone full peer, running Iroh itself: unlike watchOS, Wear OS allows
// sockets, so it syncs with every paired device as the phone does.
android {
    namespace = "io.github.emassey0135.lumenna.wear"
    compileSdk = 37

    defaultConfig {
        // The phone app's identifier, as a watch app of the same app has it.
        applicationId = "io.github.emassey0135.lumenna"
        // Wear OS 3, the oldest any current watch runs.
        minSdk = 30
        targetSdk = 37
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        // A watch's ABIs: many Wear OS watches run a 32-bit Android, and the emulator x86_64
        // on Intel machines and CI.
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }
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

dependencies {
    implementation(project(":shared"))
    val bom = platform(libs.compose.bom)
    implementation(bom)
    implementation(libs.compose.ui)
    implementation(libs.wear.compose.material3)
    implementation(libs.wear.compose.foundation)
    implementation(libs.wear.compose.navigation)
    implementation(libs.wear.input)
    implementation(libs.activity.compose)
    implementation(libs.work.runtime)

    androidTestImplementation(bom)
    androidTestImplementation(libs.compose.ui.test.junit4)
    androidTestImplementation(libs.compose.ui.test.junit4.accessibility)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.junit)
    androidTestImplementation(libs.espresso.core)
    debugImplementation(libs.compose.ui.test.manifest)
}
