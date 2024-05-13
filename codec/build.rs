#![allow(unused)]

use std::{env, fs, path::Path, process::Command};

use anyhow::anyhow;
use dotenv::dotenv;

fn join(root: &str, next: &str) -> anyhow::Result<String> {
    Ok(Path::new(root)
        .join(next)
        .to_str()
        .ok_or_else(|| anyhow!("Failed to path into string."))?
        .to_string())
}

#[allow(unused)]
fn is_exsit(dir: &str) -> bool {
    fs::metadata(dir).is_ok()
}

fn exec(command: &str, work_dir: &str) -> anyhow::Result<String> {
    let output = Command::new(if cfg!(windows) { "powershell" } else { "bash" })
        .arg(if cfg!(windows) { "-command" } else { "-c" })
        .arg(command)
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

#[cfg(target_os = "linux")]
fn find_library(name: &str) -> (Vec<String>, Vec<String>) {
    let probe = pkg_config::probe_library(name).expect(&format!(
        "You don't have pck-config or {}-dev installed.",
        name
    ));
    (
        probe
            .include_paths
            .iter()
            .map(|path| path.to_str().unwrap().to_string())
            .collect::<Vec<String>>(),
        probe
            .link_paths
            .iter()
            .map(|path| path.to_str().unwrap().to_string())
            .collect::<Vec<String>>(),
    )
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=./lib");
    println!("cargo:rerun-if-changed=./build.rs");

    let settings = Settings::build()?;
    if !is_exsit(&join(
        &settings.out_dir,
        if cfg!(target_os = "windows") {
            "yuv-windows-x86_64.lib"
        } else {
            "libyuv-linux-x86_64.a"
        },
    )?) {
        if cfg!(target_os = "windows") {
            exec("Invoke-WebRequest \
                -Uri https://github.com/mycrl/libyuv-rs/releases/download/v0.1.2/yuv-windows-x86_64.lib \
                -OutFile yuv.lib", &settings.out_dir)?;
        } else {
            exec(
                "wget \
                https://github.com/mycrl/libyuv-rs/releases/download/v0.1.2/libyuv-linux-x86_64.a \
                -O libyuv.a",
                &settings.out_dir,
            )?;
        }
    }

    if !is_exsit(&join(&settings.out_dir, "./libyuv")?) {
        exec(
            "git clone --branch stable https://chromium.googlesource.com/libyuv/libyuv",
            &settings.out_dir,
        )?;
    }

    cc::Build::new()
        .cpp(true)
        .std("c++20")
        .debug(settings.is_debug)
        .static_crt(true)
        .target(&settings.target)
        .warnings(false)
        .out_dir(&settings.out_dir)
        .file("./lib/codec.cpp")
        .file("./lib/video_encode.cpp")
        .file("./lib/video_decode.cpp")
        .file("./lib/audio_encode.cpp")
        .file("./lib/audio_decode.cpp")
        .includes(&settings.ffmpeg_include_prefix)
        .include("../common/include")
        .include(&join(&settings.out_dir, "./libyuv/include")?)
        .compile("codec");

    println!("cargo:rustc-link-search=all={}", &settings.out_dir);
    for path in &settings.ffmpeg_lib_prefix {
        println!("cargo:rustc-link-search=all={}", path);
    }

    println!("cargo:rustc-link-lib=avcodec");
    println!("cargo:rustc-link-lib=avutil");
    println!("cargo:rustc-link-lib=codec");
    println!("cargo:rustc-link-lib=yuv");
    Ok(())
}

struct Settings {
    is_debug: bool,
    target: String,
    out_dir: String,
    ffmpeg_include_prefix: Vec<String>,
    ffmpeg_lib_prefix: Vec<String>,
}

impl Settings {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn build() -> anyhow::Result<Self> {
        let _ = dotenv();
        let out_dir = env::var("OUT_DIR")?;
        let (ffmpeg_include_prefix, ffmpeg_lib_prefix) = if let (Some(include), Some(lib)) = (
            env::var("FFMPEG_INCLUDE_PREFIX").ok(),
            env::var("FFMPEG_LIB_PREFIX").ok(),
        ) {
            (vec![include], vec![lib])
        } else {
            find_ffmpeg_prefix(&out_dir)?
        };

        Ok(Self {
            out_dir,
            ffmpeg_lib_prefix,
            ffmpeg_include_prefix,
            target: env::var("TARGET")?,
            is_debug: env::var("DEBUG")
                .map(|label| label == "true")
                .unwrap_or(true),
        })
    }
}

#[cfg(target_os = "windows")]
fn find_ffmpeg_prefix(out_dir: &str) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    let ffmpeg_prefix = join(
        out_dir,
        "ffmpeg-n6.1.1-96-g1606aab99b-win64-lgpl-shared-6.1",
    )
    .unwrap();
    if !is_exsit(&ffmpeg_prefix) {
        exec(
            "Invoke-WebRequest \
                -Uri https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2024-05-12-13-21/ffmpeg-n6.1.1-96-g1606aab99b-win64-lgpl-shared-6.1.zip \
                -OutFile ffmpeg.zip",
            out_dir,
        )?;

        exec(
            "Expand-Archive -Path ffmpeg.zip -DestinationPath ./",
            out_dir,
        )?;
    }

    Ok((
        vec![join(&ffmpeg_prefix, "./include")?],
        vec![join(&ffmpeg_prefix, "./lib")?],
    ))
}

#[cfg(target_os = "linux")]
fn find_ffmpeg_prefix(out_dir: &str) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    let mut includes = Vec::new();
    let mut libs = Vec::new();

    for lib in ["libavcodec", "libavutil"] {
        let mut prefix = find_library(lib);
        includes.append(&mut prefix.0);
        libs.append(&mut prefix.1);
    }

    Ok((includes, libs))
}

#[cfg(target_os = "macos")]
fn main() {}
