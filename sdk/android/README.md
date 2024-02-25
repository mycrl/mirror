settings.gradle

```gradle
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()

        maven {
            url = uri('https://maven.pkg.github.com/mycrl/mirror')
            credentials {
                username = 'mycrl'
                password = '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'
            }
        }
    }
}
```

build.gradle

```gradle
dependencies {
    implementation 'com.github.mycrl:mirror:0.0.1'
}
```
