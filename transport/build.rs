use std::{env, fs, path::Path, process::Command};

use anyhow::{anyhow, Result};
use which::which;

fn is_exsit(dir: &str) -> bool {
    fs::metadata(dir).is_ok()
}

fn join(root: &str, next: &str) -> String {
    Path::new(root).join(next).to_str().unwrap().to_string()
}

fn exec(command: &str, work_dir: &str) -> Result<String> {
    let output = Command::new(if cfg!(windows) { "powershell" } else { "bash" })
        .arg(if cfg!(windows) { "-command" } else { "-c" })
        .arg(if cfg!(windows) {
            format!("$ProgressPreference = 'SilentlyContinue';{}", command)
        } else {
            command.to_string()
        })
        .current_dir(work_dir)
        .output()?;
    if !output.status.success() {
        Err(anyhow!("{}", unsafe {
            String::from_utf8_unchecked(output.stderr)
        }))
    } else {
        Ok(unsafe { String::from_utf8_unchecked(output.stdout) })
    }
}

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=./build.rs");

    if which("cmake").is_err() {
        panic!("
            You don't have cmake installed, compiling srt requires cmake to do it, now it's unavoidable, you need to install cmake.
                On debian/ubuntu, you can install it with `sudo apt install cmake`.
                On window, it requires you to go to the official cmake website to load the installation file.
        ");
    }

    let target = env::var("TARGET")?;
    let out_dir = env::var("OUT_DIR")?;

    if target.contains("android") {
        use_android_library(out_dir)?;
    } else {
        let srt_dir = join(&out_dir, "srt");
        if !is_exsit(&srt_dir) {
            exec(
                "git clone --branch v1.5.4 https://github.com/Haivision/srt",
                &out_dir,
            )?; // fixed 1.5.4-rc.0
        }

        use_library(srt_dir)?;
    }

    Ok(())
}

fn use_android_library(out_dir: String) -> Result<()> {
    if !is_exsit(&join(&out_dir, "libsrt.a")) {
        exec(
            "wget \
            -O libsrt.a \
            https://github.com/mycrl/third-party/releases/download/distributions/libsrt-arm64-v8a.a",
            &out_dir,
        )?;
    }

    if !is_exsit(&join(&out_dir, "libssl.a")) {
        exec(
            "wget \
            -O libssl.a \
            https://github.com/mycrl/third-party/releases/download/distributions/libssl-arm64-v8a.a",
            &out_dir,
        )?;
    }

    if !is_exsit(&join(&out_dir, "libcrypto.a")) {
        exec(
            "wget \
            -O libcrypto.a \
            https://github.com/mycrl/third-party/releases/download/distributions/libcrypto-arm64-v8a.a",
            &out_dir,
        )?;
    }

    println!("cargo:rustc-link-search=all={}", out_dir);
    println!("cargo:rustc-link-lib=static=srt");
    println!("cargo:rustc-link-lib=static=ssl");
    println!("cargo:rustc-link-lib=static=crypto");
    println!("cargo:rustc-link-lib=c++");
    Ok(())
}

#[cfg(target_os = "windows")]
fn use_library(srt_dir: String) -> Result<()> {
    if !is_exsit(&join(&srt_dir, "./Release/srt_static.lib")) {
        exec(
            "cmake \
            -DENABLE_DEBUG=OFF \
            -DCMAKE_BUILD_TYPE=Release \
            -DENABLE_APPS=OFF \
            -DENABLE_BONDING=ON \
            -DENABLE_CODE_COVERAGE=OFF \
            -DENABLE_SHARED=OFF \
            -DENABLE_ENCRYPTION=OFF \
            -DENABLE_UNITTESTS=OFF \
            -DENABLE_STDCXX_SYNC=ON \
            .",
            &srt_dir,
        )?;

        // use MultiThreaded
        for vcxproj in ["srt_static.vcxproj", "srt_virtual.vcxproj"].map(|it| join(&srt_dir, it)) {
            fs::write(
                &vcxproj,
                fs::read_to_string(&vcxproj)?.replace("MultiThreadedDLL", "MultiThreaded"),
            )?;
        }

        exec("cmake --build . --config Release", &srt_dir)?;
    }

    println!(
        "cargo:rustc-link-search=all={}",
        join(&srt_dir, "./Release")
    );

    println!("cargo:rustc-link-lib=srt_static");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn use_library(srt_dir: String) -> Result<()> {
    // linux patch
    #[cfg(target_os = "linux")]
    if !fs::read_to_string(join(&srt_dir, "CMakeLists.txt"))?
        .contains("set(CMAKE_CXX_FLAGS \"-fPIC\")")
    {
        exec(
            "sed -i '12i set(CMAKE_CXX_FLAGS \"-fPIC\")' CMakeLists.txt",
            &srt_dir,
        )?;
    }

    if !is_exsit(&join(&srt_dir, "./libsrt.a")) {
        exec(
            "./configure \
            --enable-shared=OFF \
            --use-static-libstdc++=ON \
            --enable-apps=OFF \
            --enable-debug=0 \
            --enable-encryption=OFF",
            &srt_dir,
        )?;

        exec("make", &srt_dir)?;
    }

    println!("cargo:rustc-link-search=all={}", srt_dir);
    println!("cargo:rustc-link-lib=srt");

    if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-lib=stdc++");
    } else {
        println!("cargo:rustc-link-lib=c++");
    }

    Ok(())
}
