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
        Err(anyhow!("{}", String::from_utf8(output.stderr)?))
    } else {
        Ok(String::from_utf8(output.stdout)?)
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

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=./lib");
    println!("cargo:rerun-if-changed=./build.rs");

    let settings = Settings::build()?;
    cc::Build::new()
        .cpp(false)
        .std("c17")
        .debug(settings.is_debug)
        .static_crt(true)
        .target(&settings.target)
        .warnings(false)
        .out_dir(&settings.out_dir)
        .file("./lib/devices.c")
        .includes(&settings.ffmpeg_include_prefix)
        .compile("devices");

    println!("cargo:rustc-link-search=all={}", &settings.out_dir);
    for path in &settings.ffmpeg_lib_prefix {
        println!("cargo:rustc-link-search=all={}", path);
    }

    println!("cargo:rustc-link-lib=avdevice");
    println!("cargo:rustc-link-lib=avformat");
    println!("cargo:rustc-link-lib=avutil");
    println!("cargo:rustc-link-lib=devices");
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

#[cfg(target_os = "macos")]
fn find_ffmpeg_prefix(out_dir: &str) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    let ffmpeg_prefix = exec("brew --prefix ffmpeg", out_dir)
        .expect("You don't have ffmpeg installed, please install ffmpeg: `brew install ffmpeg`.");

    Ok((
        vec![join(ffmpeg_prefix.trim(), "./include")?],
        vec![join(ffmpeg_prefix.trim(), "./lib")?],
    ))
}

#[cfg(target_os = "windows")]
fn find_ffmpeg_prefix(out_dir: &str) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    if !is_exsit(&join(&out_dir, "7z.exe").unwrap()) {
        exec(
            "Invoke-WebRequest -Uri https://www.7-zip.org/a/7zr.exe -OutFile 7z.exe",
            &out_dir,
        )
        .expect("Unable to download 7z cli exe.");
    }

    let ffmpeg_prefix = join(&out_dir, "ffmpeg-6.0-full_build-shared").unwrap();
    if !is_exsit(&ffmpeg_prefix) {
        exec(
            "Invoke-WebRequest -Uri https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-6.0-full_build-shared.7z -OutFile ffmpeg.7z; \
                     ./7z.exe x ffmpeg.7z -aoa; \
                     del ffmpeg.7z",
            &out_dir,
        )
        .expect("Unable to download ffmpeg shard release.");
    }

    Ok((
        vec![join(&ffmpeg_prefix, "./include")?],
        vec![join(&ffmpeg_prefix, "./lib")?],
    ))
}

#[cfg(target_os = "linux")]
fn find_ffmpeg_prefix(out_dir: &str) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    Ok(find_library("libavdevice"))
}
