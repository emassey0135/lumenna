plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

// The Wear OS app: Compose for Wear OS over the same core and shared code as the phone
// (`:shared`). A standalone full peer, running Iroh itself: unlike watchOS, Wear OS allows
// sockets, so it syncs with every paired device as the phone does.
android {
    namespace = "dev.emassey0135.lumenna.wear"
    compileSdk = 37

    defaultConfig {
        // The phone app's identifier, as a watch app of the same app has it.
        applicationId = "dev.emassey0135.lumenna"
        // Wear OS 3, the oldest any current watch runs.
        minSdk = 30
        targetSdk = 37
        // The workspace's, from its Cargo.toml (the root build script).
        versionCode = rootProject.extra["lumennaVersionCode"] as Int
        versionName = rootProject.extra["lumennaVersion"] as String
        resValue("string", "app_name", rootProject.extra["lumennaName"] as String)
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        // A watch's ABIs: many Wear OS watches run a 32-bit Android, and the emulator x86_64
        // on Intel machines and CI.
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }
    }

    buildTypes {
        release {
            // R8 shrinks and optimizes the Kotlin and Compose code and drops unused resources;
            // what the core reaches by name is kept by :shared's consumer rules.
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
        }
    }

    buildFeatures {
        compose = true
        // The app's name, from the workspace's metadata.
        resValues = true
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
    // BridgingManager: the phone's reminder notifications are not bridged here, since the
    // watch gives its own store's.
    implementation(libs.wear.phone.interactions)
    // The tile and the watch faces' complications, drawn from the core's glance (`Glance.kt`).
    implementation(libs.wear.tiles)
    implementation(libs.wear.protolayout)
    implementation(libs.wear.protolayout.material3)
    implementation(libs.wear.complications.data.source)

    androidTestImplementation(bom)
    androidTestImplementation(libs.compose.ui.test.junit4)
    androidTestImplementation(libs.compose.ui.test.junit4.accessibility)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.junit)
    androidTestImplementation(libs.espresso.core)
    debugImplementation(libs.compose.ui.test.manifest)
}
