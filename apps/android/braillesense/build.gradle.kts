plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

// The BrailleSense 7 app. Its package is the one Android ID that is not the workspace's
// `package_id`: the device's launcher lists `com.extra.han.*` in its Programs menu, and its screen
// reader yields the braille keys to such a package, by its name alone (`records`' metadata test).
android {
    namespace = "com.extra.han.lumenna"
    compileSdk = 37

    defaultConfig {
        applicationId = "com.extra.han.lumenna"
        // Android 15 is every BrailleSense 7; the screens use nothing newer than the shared core.
        minSdk = 28
        targetSdk = 37
        versionCode = rootProject.extra["lumennaVersionCode"] as Int
        versionName = rootProject.extra["lumennaVersion"] as String
        resValue("string", "app_name", rootProject.extra["lumennaName"] as String)
        ndk { abiFilters += listOf("arm64-v8a") }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }

    lint {
        // The activity is a ComponentActivity and uses no fragments; `:shared`'s Play Services
        // brings an old Fragment library, which this check takes for one that is in use.
        disable += "InvalidFragmentVersionForActivityResult"
    }

    buildFeatures {
        buildConfig = true
        compose = true
        resValues = true
    }
}

dependencies {
    implementation(project(":shared"))
    implementation(project(":braille"))
    implementation(project(":braillesense-sdk"))
    // `Core`'s Compose state and the activity it runs on.
    val bom = platform(libs.compose.bom)
    implementation(bom)
    implementation(libs.compose.ui)
    implementation(libs.activity.compose)
    implementation(libs.work.runtime)
    implementation(libs.androidx.core)
    testImplementation(libs.junit)
}
