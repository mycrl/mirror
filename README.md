<!--lint disable no-literal-urls-->
<div align="center">
   <h1>MIRROR</h1>
</div>
<div align="center">
  <strong>A cross-platform screen casting library implemented by rust.</strong>
</div>
<div align="center">
  <img src="https://img.shields.io/github/actions/workflow/status/mycrl/mirror/release.yml"/>
  <img src="https://img.shields.io/github/license/mycrl/mirror"/>
  <img src="https://img.shields.io/github/issues/mycrl/mirror"/>
  <img src="https://img.shields.io/github/stars/mycrl/mirror"/>
</div>
<br/>
<br/>

Pure software screen projection is different from Miracast, AirPlay, etc., which need to rely on hardware support. This project was not designed to work on a wide area network, but works well in a local area network environment.

The project is cross-platform, but the priority platforms supported are Windows and Android, Unlike a solution like DLAN, this project is more akin to airplay, so low latency is the main goal, currently the latency is controlled at around 150-250ms (with some variations on different platforms with different codecs), and maintains a highly easy to use API and very few external dependencies.

## Documentation

-   Rust: There are still some obstacles to releasing to crates.io, so for rust the documentation is being updated.
-   C/C++: This project also compiles dynamic link libraries, so for C/C++ projects, use this header file [ffi/include/mirror.h](./ffi/include/mirror.h)

## Examples

> Automated builds can be downloaded from the github release page.

-   [Android](./examples/android) - this is an android studio project.
-   [C++](./examples/cpp) - the build product is `build/bin/example-cpp`.
-   [Rust](./examples/rust) - the build product is `build/bin/example`.

## Build Instructions

#### Requirements

-   [Git](https://git-scm.com/downloads)
-   [Rust](https://www.rust-lang.org/tools/install): Rust stable toolchain.
-   C++20 or above compliant compiler. (G++/Clang/MSVC)
-   [CMake](https://cmake.org/download/): CMake 3.16 or above as a build system.
-   [Node.js](https://nodejs.org/en/download): Node.js 16 or above as a auto build script.
-   [Python3](https://www.python.org/downloads/): Python3 is optional and required for Android Studio projects.

##### Linux (Ubuntu/Debian)

> For Linux, you need to install additional dependencies to build SRT and other.

```sh
sudo apt-get update
sudo apt-get install tclsh pkg-config cmake libssl-dev build-essential libavcodec-dev libavdevice-dev libavformat-dev libasound2-dev libsdl2-dev libmfx-dev libva-dev v4l-utils
```

---

#### Build

Examples and SDK library files can be automatically packaged by running an automatic compilation script.

```sh
npm run build:release
```

The Release version is compiled by default. If you need the Debug version, just run `npm run build:debug`.  
For android, there is no need to manually call compilation. You can directly use Android Studio to open [./examples/android](./examples/android).

## License

[LGPL](./LICENSE) Copyright (c) 2024 mycrl.
