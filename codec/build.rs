use std::{env, fs, path::Path, process::Command};

fn join(root: &str, next: &str) -> anyhow::Result<String> {
    Ok(Path::new(root)
        .join(next)
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Failed to path into string."))?
        .to_string())
}

fn is_exsit(dir: &str) -> bool {
    fs::metadata(dir).is_ok()
}

fn exec(command: &str, work_dir: &str) -> anyhow::Result<String> {
    let output = Command::new(if cfg!(target_os = "windows") {
        "powershell"
    } else {
        "bash"
    })
    .arg(if cfg!(target_os = "windows") {
        "-command"
    } else {
        "-c"
    })
    .arg(if cfg!(target_os = "windows") {
        format!("$ProgressPreference = 'SilentlyContinue';{}", command)
    } else {
        command.to_string()
    })
    .current_dir(work_dir)
    .output()?;
    if !output.status.success() {
        Err(anyhow::anyhow!("{}", unsafe {
            String::from_utf8_unchecked(output.stderr)
        }))
    } else {
        Ok(unsafe { String::from_utf8_unchecked(output.stdout) })
    }
}

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=./lib");
    println!("cargo:rerun-if-changed=./build.rs");

    let settings = Settings::build()?;
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
        .includes(&settings.libyuv_include_prefix)
        .include("../common/include")
        .define(
            if cfg!(target_os = "windows") {
                "WIN32"
            } else if cfg!(target_os = "linux") {
                "LINUX"
            } else {
                "MACOS"
            },
            None,
        )
        .compile("codec");

    for path in &settings.ffmpeg_lib_prefix {
        println!("cargo:rustc-link-search=all={}", path);
    }

    for path in &settings.libyuv_lib_prefix {
        println!("cargo:rustc-link-search=all={}", path);
    }

    println!("cargo:rustc-link-search=all={}", &settings.out_dir);
    println!("cargo:rustc-link-lib=avcodec");
    println!("cargo:rustc-link-lib=avutil");
    println!("cargo:rustc-link-lib=codec");
    println!("cargo:rustc-link-lib=yuv");

    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=c++");
    } else if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-lib=stdc++");
    }

    Ok(())
}

#[derive(Debug)]
struct Settings {
    is_debug: bool,
    target: String,
    out_dir: String,
    ffmpeg_include_prefix: Vec<String>,
    ffmpeg_lib_prefix: Vec<String>,
    libyuv_include_prefix: Vec<String>,
    libyuv_lib_prefix: Vec<String>,
}

impl Settings {
    fn build() -> anyhow::Result<Self> {
        let out_dir = env::var("OUT_DIR")?;
        let is_debug = env::var("DEBUG")
            .map(|label| label == "true")
            .unwrap_or(true);
        let (ffmpeg_include_prefix, ffmpeg_lib_prefix) = find_ffmpeg_prefix(&out_dir, is_debug)?;
        let (libyuv_include_prefix, libyuv_lib_prefix) = find_libyuv_prefix(&out_dir)?;
        Ok(Self {
            out_dir,
            is_debug,
            ffmpeg_lib_prefix,
            ffmpeg_include_prefix,
            libyuv_include_prefix,
            libyuv_lib_prefix,
            target: env::var("TARGET")?,
        })
    }
}

fn find_ffmpeg_prefix(out_dir: &str, is_debug: bool) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    if cfg!(target_os = "macos") {
        let prefix = exec("brew --prefix ffmpeg@6", out_dir)?.replace('\n', "");

        Ok((
            vec![join(&prefix, "./include")?],
            vec![join(&prefix, "./lib")?],
        ))
    } else {
        let prefix = join(out_dir, "ffmpeg").unwrap();
        if !is_exsit(&prefix) {
            if cfg!(target_os = "windows") {
                exec(
                    &format!(
                        "Invoke-WebRequest -Uri https://github.com/mycrl/mirror/releases/download/distributions/ffmpeg-windows-x64-{}.zip -OutFile ffmpeg.zip", 
                        if is_debug { "debug" } else { "release" }
                    ),
                    out_dir,
                )?;

                exec(
                    "Expand-Archive -Path ffmpeg.zip -DestinationPath ./",
                    out_dir,
                )?;
            } else {
                exec(
                    "wget https://github.com/mycrl/mirror/releases/download/distributions/ffmpeg-linux-x64-release.zip -O ffmpeg.zip",
                    out_dir,
                )?;

                exec("unzip ffmpeg.zip", out_dir)?;
            }
        }

        Ok((
            vec![join(&prefix, "./include")?],
            vec![join(&prefix, "./lib")?],
        ))
    }
}

fn find_libyuv_prefix(out_dir: &str) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    if !is_exsit(&join(&out_dir, "./libyuv")?) {
        exec(
            "git clone --branch stable https://chromium.googlesource.com/libyuv/libyuv",
            &out_dir,
        )?;
    }

    if cfg!(target_os = "windows") {
        if !is_exsit(&join(&out_dir, "yuv.lib")?) {
            exec(
                "Invoke-WebRequest -Uri https://github.com/mycrl/mirror/releases/download/distributions/yuv-windows-x64.lib -OutFile yuv.lib", 
                &out_dir
            )?;
        }
    } else {
        if !is_exsit(&join(&out_dir, "libyuv.a")?) {
            exec(
                &format!(
                    "wget https://github.com/mycrl/mirror/releases/download/distributions/libyuv-{}-{}.a -O libyuv.a", 
                    if cfg!(target_os = "macos") { "macos" } else { "linux" },
                    if cfg!(target_os = "macos") { "arm64" } else { "x64" },
                ),
                &out_dir
            )?;
        }
    }

    Ok((
        vec![join(&out_dir, "./libyuv/include")?],
        vec![out_dir.to_string()],
    ))
}
