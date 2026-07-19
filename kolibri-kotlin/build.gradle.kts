// Root build script. Per-module config lives in library/ and example/.
plugins {
    kotlin("jvm") version "2.0.21" apply false
}

allprojects {
    repositories {
        mavenCentral()
    }
}
