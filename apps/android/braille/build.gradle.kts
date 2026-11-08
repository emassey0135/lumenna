plugins {
    alias(libs.plugins.android.library)
}

// What the braille apps' Kotlin really shares (the Orbit app and the BrailleSense app): a vendor
// service's ordered worker, holding the keys and display only while in front, and handing a
// decoded key to the app. Their device code stays in each module. What the three braille apps
// share beyond Kotlin is the core's (`lumenna_app`).
android {
    namespace = "dev.emassey0135.lumenna.braille"
    compileSdk = 37

    defaultConfig {
        minSdk = 28
    }

    testOptions {
        // Only the log touches Android here, which then does nothing.
        unitTests.isReturnDefaultValues = true
    }
}

dependencies {
    testImplementation(libs.junit)
}
