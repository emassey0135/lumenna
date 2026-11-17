plugins {
    // No version: the library's build at the root has the Android plugin on the classpath already.
    id("com.android.application")
}

// The README's quick start, built: a list of notes to add, edit and delete, on the SDK's stack.
android {
    namespace = "dev.emassey0135.orbit.sample"
    compileSdk = 37

    defaultConfig {
        applicationId = "dev.emassey0135.orbit.sample"
        // The Orbit devices run Android 11, and stay there.
        minSdk = 30
        targetSdk = 30
        versionCode = 1
        versionName = "1.0"
    }

    lint {
        // Android 11 is what these devices run; an app for them is sideloaded.
        disable += setOf("ExpiredTargetSdkVersion", "OldTargetApi")
    }
}

dependencies {
    implementation(project(":"))
}
