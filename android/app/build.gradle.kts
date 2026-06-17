// ThunderCrab Android — :app module build script (BUILD group).
// AVP-2: UNVERIFIED — UNSAFE — nothing here is SHIP-DECISION.
//
// Pinned to the frozen build matrix: JDK 21 (Temurin), AGP 8.5.2, Gradle 8.9,
// Kotlin 2.0.21 (+ compose plugin), Compose BOM 2024.09.00, NDK 26.3.11579264.
//
// This module also wires the UniFFI pipeline with hand-rolled Exec tasks (no
// gradle plugin): cargo-ndk builds libthundercrab_ffi.so per ABI, then
// uniffi-bindgen generates the Kotlin surface (package uniffi.thundercrab_ffi).
// Both the .so files and the generated Kotlin are BUILD ARTIFACTS, not source.
import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.compose.compiler)
}

// ABI set is parameterized for fast iteration (one ABI builds far quicker):
//   ./gradlew assembleDebug -Ptc.abis=arm64-v8a
// Default builds all four shipping ABIs.
val tcAbis: List<String> = (findProperty("tc.abis") as String?)
    ?.split(",")?.map { it.trim() }?.filter { it.isNotEmpty() }
    ?: listOf("arm64-v8a", "armeabi-v7a", "x86_64", "x86")

android {
    namespace = "com.plausiden.thundercrab"
    compileSdk = 34
    ndkVersion = "26.3.11579264" // PINNED r26 (see 16KB risk note in gradle plan)

    defaultConfig {
        applicationId = "com.plausiden.thundercrab"
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk {
            abiFilters += tcAbis
        }
    }

    signingConfigs {
        getByName("debug") {
            // Explicit debug signingConfig (auto-generated ~/.android/debug.keystore).
            storeFile = file(System.getProperty("user.home") + "/.android/debug.keystore")
            storePassword = "android"
            keyAlias = "androiddebugkey"
            keyPassword = "android"
        }
    }

    buildTypes {
        getByName("debug") {
            signingConfig = signingConfigs.getByName("debug")
            isMinifyEnabled = false
        }
        release {
            isMinifyEnabled = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            // Scaffold only — signs release with the debug key. This is NOT a
            // real release key (AVP-2: UNVERIFIED — UNSAFE).
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    kotlin { jvmToolchain(21) } // JDK 21 Temurin toolchain

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }

    buildFeatures { compose = true } // NO composeOptions{} (Kotlin 2.0 compose plugin)

    sourceSets["main"].kotlin.srcDir("src/main/kotlin")
    sourceSets["test"].kotlin.srcDir("src/test/kotlin")

    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.kotlinx.coroutines.android)

    val composeBom = platform(libs.androidx.compose.bom)
    implementation(composeBom)
    androidTestImplementation(composeBom)

    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.material3)
    debugImplementation(libs.androidx.compose.ui.tooling)

    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose) // viewModel()
    implementation(libs.androidx.lifecycle.runtime.compose)   // collectAsStateWithLifecycle()
    implementation(libs.androidx.navigation.compose)

    // MANDATORY @aar — the desktop jar lacks the bundled native lib and fails at
    // runtime. UniFFI loads our .so through JNA, so this must be the aar variant.
    implementation(variantOf(libs.jna) { artifactType("aar") })

    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
}

// ---------------------------------------------------------------------------
// UniFFI wiring: manual Exec tasks (no plugin). From the canonical recipe.
//
// The generated Kotlin lands at:
//   app/build/generated/source/uniffi/java/uniffi/thundercrab_ffi/*.kt
// and the per-ABI native libs at:
//   app/src/main/jniLibs/<abi>/libthundercrab_ffi.so
// Both are build artifacts (git-ignored), NOT owned source files.
//
// soname is libthundercrab_ffi.so (crate thundercrab-ffi, crate-type lib/cdylib/
// staticlib); the generated package is uniffi.thundercrab_ffi.
// ---------------------------------------------------------------------------
val rustWorkspaceDir = rootProject.file("../") // Cargo workspace root = repo root
val ffiCrate = "thundercrab-ffi"
val soName = "libthundercrab_ffi.so"
val jniLibsDir = layout.projectDirectory.dir("src/main/jniLibs").asFile
val uniffiGenDir = layout.buildDirectory.dir("generated/source/uniffi/java")

val cargoNdkBuild = tasks.register<Exec>("cargoNdkBuildThundercrab") {
    group = "uniffi"
    description = "Cross-compiles libthundercrab_ffi.so for all Android ABIs via cargo-ndk."
    workingDir = rustWorkspaceDir
    environment("ANDROID_NDK_HOME", android.ndkDirectory.absolutePath)
    // If cargo / cargo-ndk are not on the Gradle daemon PATH, uncomment:
    // environment("PATH", "${System.getProperty("user.home")}/.cargo/bin:" + System.getenv("PATH"))
    // 16KB page-size mitigation for NDK r26 (see risk note in the gradle plan):
    environment("RUSTFLAGS", "-C link-arg=-Wl,-z,max-page-size=16384")
    commandLine(
        listOf("cargo", "ndk") +
            tcAbis.flatMap { listOf("-t", it) } +
            listOf(
                "-P", "26", // cargo-ndk: -P/--platform = Android API level (lowercase -p is cargo's --package)
                "-o", jniLibsDir.absolutePath,
                "build", "--release",
                "-p", ffiCrate, // load-bearing: build only the FFI crate
                "--lib",        // load-bearing: only the cdylib/lib target
            )
    )
    inputs.dir(rustWorkspaceDir.resolve(ffiCrate).resolve("src"))
    outputs.dir(jniLibsDir)
}

val generateUniffiBindings = tasks.register<Exec>("generateUniffiBindings") {
    group = "uniffi"
    description = "Generates the Kotlin UniFFI bindings from the built arm64 .so (library mode)."
    workingDir = rustWorkspaceDir
    dependsOn(cargoNdkBuild)
    val arm64So = jniLibsDir.resolve("arm64-v8a").resolve(soName)
    commandLine(
        "cargo", "run", "-p", ffiCrate, "--bin", "uniffi-bindgen", "--",
        "generate", "--library", arm64So.absolutePath,
        "--language", "kotlin",
        "--out-dir", uniffiGenDir.get().asFile.absolutePath,
    )
    inputs.file(arm64So)
    outputs.dir(uniffiGenDir)
}

// Register the generated Kotlin as a source directory and make compilation +
// packaging depend on the UniFFI tasks running first.
android.sourceSets.getByName("main").java.srcDir(uniffiGenDir)
tasks.withType<KotlinCompile>().configureEach { dependsOn(generateUniffiBindings) }
tasks.named("preBuild").configure { dependsOn(cargoNdkBuild) } // .so present before AGP packages
