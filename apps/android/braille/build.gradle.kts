plugins {
    alias(libs.plugins.android.library)
}

// What the braille apps' Kotlin really shares: the dialog controller over the core's description of
// a form, which the Orbit app and the BrailleSense app both work with their device's keys. What is
// the device's own (holding the keys while in front, its services, its lists, menus and dialogs'
// keys) is each device's SDK's. What the three braille apps share beyond Kotlin is the core's
// (`lumenna_app`).
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
    // The core's records and `control_event`, which the dialog controller works over.
    implementation(project(":shared"))
    testImplementation(libs.junit)
}
