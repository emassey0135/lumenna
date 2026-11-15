plugins {
    alias(libs.plugins.android.library)
}

// An unofficial SDK for Orbit Research's Android devices, the Orbit Speak and the Orbit Player:
// everything about Orbit itself, and nothing of Lumenna's, so another app (Satellite) can use it
// and it can move to a repository of its own. It depends on nothing but Android.
android {
    namespace = "dev.emassey0135.orbit"
    compileSdk = 37

    defaultConfig {
        // The Orbit devices run Android 11, and stay there.
        minSdk = 30
    }

    buildFeatures {
        aidl = true
    }

    testOptions {
        // Only the log touches Android in what is tested, which then does nothing.
        unitTests.isReturnDefaultValues = true
    }
}

dependencies {
    testImplementation(libs.junit)
}
