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
    if cfg!(target_os = "macos") {
        return Ok(());
    }

    println!("cargo:rerun-if-changed=./lib");
    println!("cargo:rerun-if-changed=./build.rs");

    let settings = Settings::build()?;
    if !is_exsit(&join(
        &settings.out_dir,
        if cfg!(target_os = "windows") {
            "yuv.lib"
        } else {
            "libyuv.a"
        },
    )?) {
        if cfg!(target_os = "windows") {
            exec("Invoke-WebRequest \
                -Uri https://github.com/mycrl/mirror/releases/download/distributions/yuv-windows-x64.lib \
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
        .define(
            if cfg!(target_os = "windows") {
                "WIN32"
            } else {
                "LINUX"
            },
            None,
        )
        .compile("codec");

    println!("cargo:rustc-link-search=all={}", &settings.out_dir);
    for path in &settings.ffmpeg_lib_prefix {
        println!("cargo:rustc-link-search=all={}", path);
    }

    println!("cargo:rustc-link-lib=yuv");
    println!("cargo:rustc-link-lib=avcodec");
    println!("cargo:rustc-link-lib=avutil");
    println!("cargo:rustc-link-lib=codec");
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
        let out_dir = env::var("OUT_DIR")?;
        let is_debug = env::var("DEBUG")
            .map(|label| label == "true")
            .unwrap_or(true);
        let (ffmpeg_include_prefix, ffmpeg_lib_prefix) = if let (Some(include), Some(lib)) = (
            env::var("FFMPEG_INCLUDE_PREFIX").ok(),
            env::var("FFMPEG_LIB_PREFIX").ok(),
        ) {
            (vec![include], vec![lib])
        } else {
            find_ffmpeg_prefix(&out_dir, is_debug)?
        };

        Ok(Self {
            out_dir,
            is_debug,
            ffmpeg_lib_prefix,
            ffmpeg_include_prefix,
            target: env::var("TARGET")?,
        })
    }
}

#[allow(unused_variables)]
fn find_ffmpeg_prefix(out_dir: &str, is_debug: bool) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    let ffmpeg_prefix = join(out_dir, "ffmpeg").unwrap();
    if !is_exsit(&ffmpeg_prefix) {
        #[cfg(target_os = "windows")]
        {
            exec(
                &format!("Invoke-WebRequest \
                    -Uri https://github.com/mycrl/mirror/releases/download/distributions/ffmpeg-windows-x64-{}.zip \
                    -OutFile ffmpeg.zip", if is_debug { "debug" } else { "release" }),
                out_dir,
            )?;

            exec(
                "Expand-Archive -Path ffmpeg.zip -DestinationPath ./",
                out_dir,
            )?;
        }

        #[cfg(target_os = "linux")]
        {
            exec("wget \
                https://github.com/mycrl/mirror/releases/download/distributions/ffmpeg-linux-x64-debug.tar.xz \
                -O ffmpeg.tar.xz", out_dir)?;
            exec("tar xvf ./ffmpeg.tar.xz", out_dir)?;
        }
    }

    Ok((
        vec![join(&ffmpeg_prefix, "./include")?],
        vec![join(&ffmpeg_prefix, "./lib")?],
    ))
}
