plugins {
    application
}

java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(17)
    }
}

sourceSets {
    main {
        java.srcDir(rootProject.file("../dist/java"))
        resources {
            srcDir(rootProject.file("../dist/java"))
            include("native/**")
        }
    }
}

application {
    mainClass = "dev.whalefrommars.examples.java.Main"
}
