plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    // App Functions are declared by annotations the library's compiler reads.
    alias(libs.plugins.ksp)
}

android {
    namespace = "dev.emassey0135.lumenna"
    compileSdk = 37

    defaultConfig {
        applicationId = "dev.emassey0135.lumenna"
        minSdk = 28
        targetSdk = 37
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        // A phone's ABIs: the core is built for 32-bit ARM too, for watches (build-core.sh).
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
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
    }
}

ksp {
    arg("appfunctions:generateV1Xml", "true")
}

dependencies {
    val bom = platform(libs.compose.bom)
    implementation(bom)
    implementation(libs.compose.ui)
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.activity.compose)
    implementation(libs.work.runtime)
    // Gemini's App Functions (`functions/`), and the compiler that writes their service and the
    // XML the system indexes; the v1 XML is what Android 16's indexer reads, the other Android 17's.
    implementation(libs.appfunctions)
    ksp(libs.appfunctions.compiler)
    // The system's search index (`SearchIndex`), which Android 12 and later keep.
    implementation(libs.appsearch)
    implementation(libs.appsearch.platform.storage)
    // The home screen's widgets (`widgets/`).
    implementation(libs.glance.appwidget)
    implementation(libs.glance.material3)
    // The core, its bindings and what the watch app shares (`:shared`).
    implementation(project(":shared"))

    androidTestImplementation(bom)
    testImplementation(libs.junit)
    androidTestImplementation(libs.compose.ui.test.junit4)
    androidTestImplementation(libs.compose.ui.test.junit4.accessibility)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.junit)
    // Compose's test library brings Espresso 3.5, which calls an InputManager method Android
    // 17 no longer has; every test failed before reaching the app.
    androidTestImplementation(libs.espresso.core)
    androidTestImplementation(libs.work.testing)
    debugImplementation(libs.compose.ui.test.manifest)
}
