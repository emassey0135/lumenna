plugins {
    // The Android Gradle plugin is on the classpath already, from the library at the root.
    id("com.android.application")
}

// The README's quick start, built: a list of fruit with Add and Delete, over the screen stack. Its
// package is `com.extra.han.*`, the only kind the device's screen reader yields the braille keys to.
android {
    namespace = "com.extra.han.sample"
    compileSdk = 37

    defaultConfig {
        applicationId = "com.extra.han.sample"
        minSdk = 28
        targetSdk = 37
        versionCode = 1
        versionName = "1.0"
        // Every BrailleSense 7 is arm64, and liblouis is built for it alone.
        ndk { abiFilters += listOf("arm64-v8a") }
    }

    buildFeatures {
        buildConfig = true
    }
}

dependencies {
    // The SDK is the root project of its own build.
    implementation(project(":"))
}
