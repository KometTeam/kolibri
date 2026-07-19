plugins {
    kotlin("jvm")
    `java-library`
}

dependencies {
    api("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.9.0")
    testImplementation(kotlin("test"))
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.9.0")
}

kotlin {
    jvmToolchain(17)
}

// So `gradle test` / `gradle run` can find libkolibri_kotlin.{dylib,so} built by
// build-rust.sh into rust/target/<profile>/. Override with -Dkolibri.native.dir.
val nativeDir = providers.gradleProperty("kolibri.native.dir")
    .orElse(layout.projectDirectory.dir("../rust/target/release").asFile.absolutePath)

tasks.withType<Test>().configureEach {
    useJUnitPlatform()
    systemProperty("java.library.path", nativeDir.get())
}
