plugins {
    id("com.android.library") version "9.2.1"
    id("org.jetbrains.kotlin.android") version "2.3.10"
}

android {
    namespace = "dev.quictunnel.mobile"
    compileSdk = 37

    defaultConfig {
        minSdk = 23
        consumerProguardFiles("consumer-rules.pro")
    }

    sourceSets {
        named("main") {
            java.srcDirs("src/main/java", "src/main/uniffi/kotlin")
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
}

dependencies {
    api("androidx.webkit:webkit:1.14.0")
    api("net.java.dev.jna:jna:5.17.0@aar")
}
