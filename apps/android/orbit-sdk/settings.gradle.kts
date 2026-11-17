// The SDK built on its own: the library at this directory's root, and the sample app beside it.
// An app's own build may instead include this directory as a module; its settings are then this
// file's place, and this one is not read.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "orbit-sdk"
include(":sample")
