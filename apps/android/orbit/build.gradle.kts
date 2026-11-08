plugins {
    alias(libs.plugins.android.application)
}

// The Orbit Speak and the Orbit Player: Orbit Research's Android 11 devices, self-voicing through
// Orbit's own screen reader, over the same core and shared code as the phone (`:shared`). A module
// of its own since the self-voicing flag (`orbit.self_voicing`) is static, and in the phone app it
// would silence BackTalk there.
android {
    namespace = "dev.emassey0135.lumenna.orbit"
    compileSdk = 37

    defaultConfig {
        // The phone's: the same app on devices that never run both. Not `com.orbit.`, and not
        // holding "inputmethod", which Orbit's routing would misread.
        applicationId = rootProject.extra["lumennaPackageId"] as String
        // The Orbit devices run Android 11, and stay there.
        minSdk = 30
        targetSdk = 30
        versionCode = rootProject.extra["lumennaVersionCode"] as Int
        versionName = rootProject.extra["lumennaVersion"] as String
        resValue("string", "app_name", rootProject.extra["lumennaName"] as String)
        // Both devices are MediaTek 64-bit.
        ndk { abiFilters += listOf("arm64-v8a") }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
        }
    }

    buildFeatures {
        aidl = true
        resValues = true
    }

    lint {
        // Android 11 is what these devices run; this app is sideloaded or from GitHub's releases.
        // The activity is a ComponentActivity: there is no Fragment to be too old.
        disable += setOf("ExpiredTargetSdkVersion", "OldTargetApi", "InvalidFragmentVersionForActivityResult")
    }

    testOptions {
        // The device layer uses Android classes only in passing (the log), which then do nothing.
        unitTests.isReturnDefaultValues = true
    }
}

dependencies {
    implementation(project(":shared"))
    implementation(project(":braille"))
    // ComponentActivity, which shared's `Front` is written over.
    implementation(libs.activity.compose)
    implementation(libs.work.runtime)
    testImplementation(libs.junit)
}
