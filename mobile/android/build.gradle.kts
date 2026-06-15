plugins {
    id("com.android.library") version "9.2.1"
}

android {
    namespace = "dev.mobilecode.connect.mobile"
    compileSdk = 36

    defaultConfig {
        minSdk = 23
        consumerProguardFiles("consumer-rules.pro")
    }

    sourceSets {
        named("main") {
            java.srcDirs("src/main/java")
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
}

dependencies {
    api("androidx.webkit:webkit:1.14.0")
    api("net.java.dev.jna:jna:5.17.0@aar")
}
