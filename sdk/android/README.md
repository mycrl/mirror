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
                username = System.getenv('GITHUB_USERNAME')
                password = System.getenv('GITHUB_TOKEN')
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
