// The SDK as a build of its own: the library at the root, and a sample app over it. Inside an app's
// build it is one module (`include(":braillesense-sdk")`), and this file is not read; its build file
// takes the same plugin aliases from that build's version catalog.
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

rootProject.name = "braillesense-sdk"
include(":sample")
