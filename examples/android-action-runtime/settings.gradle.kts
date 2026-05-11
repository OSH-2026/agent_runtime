pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
    plugins {
        id("com.android.application") version "8.4.2" apply false
        id("com.android.library") version "8.4.2" apply false
        kotlin("android") version "1.9.24" apply false
        kotlin("plugin.serialization") version "1.9.24" apply false
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "android-action-runtime"

include(":app")
include(":runtime")

project(":runtime").projectDir = file("../../kotlin/kotlin-actions-runtime")
