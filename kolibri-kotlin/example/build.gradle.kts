plugins {
    kotlin("jvm")
    application
}

dependencies {
    implementation(project(":library"))
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.9.0")
}

kotlin {
    jvmToolchain(17)
}

application {
    mainClass.set("ru.kolibri.example.HandshakeKt")
    // Point the JVM at the native lib built by ../build-rust.sh.
    val nativeDir = providers.gradleProperty("kolibri.native.dir")
        .orElse(layout.projectDirectory.dir("../rust/target/release").asFile.absolutePath)
    applicationDefaultJvmArgs = listOf("-Djava.library.path=${nativeDir.get()}")
}
