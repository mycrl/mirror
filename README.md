<!--lint disable no-literal-urls-->
<div align="center">
  <h1>mirror</h1>
</div>
<br/>
<div align="center">
  <strong>Cross-platform casting SDK, support Android, Windows, Linux</strong>
</div>
<div align="center">
  <sup>Low-latency transport protocols use [SRT](https://github.com/Haivision/srt)</sup></br>
  <sup>Video: H264, Audio: Opus</sup>
</div>
<br/>
<br/>

This is a screencasting SDK that does not rely on third-party services. It includes functions such as automatic discovery, service grouping, and automatic reconnection. It only supports LANs with good network conditions.


## Features

* Prefer hardware codecs.
* Use UDP multicast to transmit audio and video streams.
* Support Android, Windows, Linux.
* Supports Intel QSV, AMD AMF, Nvidia EVENC, MediaCodec.


## Examples

* [android](./examples/android) - Record the screen and broadcast audio and video streams, and support automatic discovery of other clients.
* [desktop](./examples/desktop/) - Record the screen and broadcast audio and video streams. Automatic discovery is not supported. The port address is fixed in the example.


## Prerequisites

* [rust](https://www.rust-lang.org/tools/install) - The main language used in the project.
* [cmake](https://cmake.org/download/) - Required when compiling C++ projects and dependencies.

#### Android

* [python3](https://www.python.org/downloads/) - cargo gradle requires python environment.

#### Windows

* [node.js](https://nodejs.org/en/download) - Automatically compiling and packaging scripts requires the node.js environment. <sup>Optional</sup>


## Building

Examples and SDK library files can be automatically packaged by running an automatic compilation script.

```sh
npm run build
```

The Debug version is compiled by default. If you need the Release version, just run `npm run build:release`.  
For android, there is no need to manually call compilation. You can directly use Android Studio to open `examples/android`.

#### Example

For Windows or Linux examples, you can compile them separately.

First you need to build the dynamic library:

```sh
cargo build
```

Next, enter the example directory and use cmake to generate and compile:

```sh
cd examples/desktop
mkdir build
cd build
cmake ..
cmake --build .
```


## License
[MIT](./LICENSE) Copyright (c) 2022 Mr.Panda.
