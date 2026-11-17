plugins {
    alias(libs.plugins.android.library)
    // The API reference, as HTML (`./gradlew dokkaGenerate`, into build/dokka/html). Its version is
    // written here rather than in a catalog, so the build file is the same whether this is built on
    // its own or as a module of an app's build.
    id("org.jetbrains.dokka") version "2.3.0"
}

// An unofficial SDK for Orbit Research's Android devices, the Orbit Speak and the Orbit Player:
// everything about Orbit itself, and nothing of any one app's. It depends on nothing but Android.
// This file serves both builds: this directory's own (settings.gradle.kts, with the sample), and an
// app's that includes it as a module, whose version catalog has the same `libs` aliases.
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

kotlin {
    // Every declaration says whether it is the SDK's API or its own, so nothing is public by accident.
    explicitApi()
}

dokka {
    moduleName.set("orbit-sdk")
    dokkaSourceSets.configureEach {
        includes.from("docs/module.md")
        // The AIDL's generated stubs are how the SDK reaches Orbit, not its API.
        suppressGeneratedFiles.set(true)
    }
}

dependencies {
    testImplementation(libs.junit)
}
