<!--lint disable no-literal-urls-->
<div align="center">
   <h1>MIRROR</h1>
</div>
<div align="center">
  <strong>A cross-platform screen casting library implemented by rust.</strong>
</div>
<div align="center">
  <img src="https://img.shields.io/github/actions/workflow/status/mycrl/mirror/release.yaml?branch=main"/>
  <img src="https://img.shields.io/github/license/mycrl/mirror"/>
  <img src="https://img.shields.io/github/issues/mycrl/mirror"/>
  <img src="https://img.shields.io/github/stars/mycrl/mirror"/>
</div>
<br/>
<br/>

Pure software screen projection is different from Miracast, AirPlay, etc., which need to rely on hardware support. This project was not designed to work on a wide area network, but works well in a local area network environment.

Unlike a solution like DLAN, this project is more akin to airplay, so low latency is the main goal, currently the latency is controlled at around 150-250ms (with some variations on different platforms with different codecs), and maintains a highly easy to use API and very few external dependencies.

The project is cross-platform, but the priority platforms supported are Windows and Android.

<br/>
<br/>

## Demonstration Video

<div align="center">
    <video src="./demonstrations.mp4" width="300"></video>
</div>

## Documentation

The documentation is still being updated, for C/C++ projects, check out this header file: [mirror.h](./ffi/include/mirror.h)

## Build Instructions

#### Requirements

- [Git](https://git-scm.com/downloads)
- [Rust](https://www.rust-lang.org/tools/install): Rust stable toolchain.
- C++20 or above compliant compiler. (G++/Clang/MSVC)
- [CMake](https://cmake.org/download/): CMake 3.16 or above as a build system.
- [Node.js](https://nodejs.org/en/download): Node.js 16 or above as a auto build script.
- [Python3](https://www.python.org/downloads/): Python3 is optional and required for Android Studio projects.

##### Linux (Ubuntu/Debian)

> For Linux, you need to install additional dependencies to build SRT and other.

```sh
sudo apt-get update
sudo apt-get install tclsh pkg-config cmake libssl-dev build-essential libasound2-dev libsdl2-dev libmfx-dev v4l-utils
```

---

#### Build

Examples and SDK library files can be automatically packaged by running an automatic compilation script.

```sh
npm run build
```

The Debug version is compiled by default. If you need the Release version, just run `npm run build:release`.  
For android, there is no need to manually call compilation. You can directly use Android Studio to open [./examples/android](./examples/android).

If you don't need to build the examples, just build the dynamic library:

```sh
cargo build --release
```

## License

[GPL](./LICENSE) Copyright (c) 2024 Lazy Panda.
