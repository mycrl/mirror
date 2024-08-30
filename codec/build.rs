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

    let target = env::var("TARGET")?;
    let out_dir = env::var("OUT_DIR")?;
    let is_debug = env::var("DEBUG")
        .map(|label| label == "true")
        .unwrap_or(true);

    let (ffmpeg_include_prefix, ffmpeg_lib_prefix) = find_ffmpeg_prefix(&out_dir, is_debug)?;
    let (libyuv_include_prefix, libyuv_lib_prefix) = find_libyuv_prefix(&out_dir)?;

    if !is_exsit(&join(&out_dir, "./media-sdk")?) {
        exec(
            "git clone https://github.com/Intel-Media-SDK/MediaSDK media-sdk",
            &out_dir,
        )?;
    }

    cc::Build::new()
        .cpp(true)
        .std("c++20")
        .debug(is_debug)
        .static_crt(true)
        .target(&target)
        .warnings(false)
        .out_dir(&out_dir)
        .file("./lib/codec.cpp")
        .file("./lib/h264.cpp")
        .file("./lib/opus.cpp")
        .includes(&ffmpeg_include_prefix)
        .includes(&libyuv_include_prefix)
        .include(join(&out_dir, "./media-sdk/api/include")?)
        .include("../frame/include")
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

    for path in &ffmpeg_lib_prefix {
        println!("cargo:rustc-link-search=all={}", path);
    }

    for path in &libyuv_lib_prefix {
        println!("cargo:rustc-link-search=all={}", path);
    }

    println!("cargo:rustc-link-search=all={}", &out_dir);
    println!("cargo:rustc-link-lib=avcodec");
    println!("cargo:rustc-link-lib=avutil");
    println!("cargo:rustc-link-lib=codec");
    println!("cargo:rustc-link-lib=yuv");

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=c++");
    }

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=stdc++");
    }

    Ok(())
}

fn find_ffmpeg_prefix(out_dir: &str, is_debug: bool) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    if cfg!(target_os = "macos") {
        let prefix = exec("brew --prefix ffmpeg@6", out_dir)?.replace('\n', "");

        Ok((
            vec![join(&prefix, "./include")?],
            vec![join(&prefix, "./lib")?],
        ))
    } else if cfg!(target_os = "windows") {
        let prefix = join(out_dir, "ffmpeg").unwrap();
        if !is_exsit(&prefix) {
            exec(
                    &format!(
                        "Invoke-WebRequest -Uri https://github.com/mycrl/third-party/releases/download/distributions/ffmpeg-windows-x64-{}.zip -OutFile ffmpeg.zip", 
                        if is_debug { "debug" } else { "release" }
                    ),
                    out_dir,
                )?;

            exec(
                "Expand-Archive -Path ffmpeg.zip -DestinationPath ./",
                out_dir,
            )?;
        }

        Ok((
            vec![join(&prefix, "./include")?],
            vec![join(&prefix, "./lib")?],
        ))
    } else {
        let prefix = join(out_dir, "ffmpeg").unwrap();
        if !is_exsit(&prefix) {
            exec(
                "wget https://github.com/mycrl/third-party/releases/download/distributions/ffmpeg-linux-x64-release.zip -O ffmpeg.zip",
                out_dir,
            )?;

            exec("unzip ffmpeg.zip", out_dir)?;
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
                "Invoke-WebRequest -Uri https://github.com/mycrl/third-party/releases/download/distributions/yuv-windows-x64.lib -OutFile yuv.lib", 
                &out_dir
            )?;
        }
    } else {
        if !is_exsit(&join(&out_dir, "libyuv.a")?) {
            exec(
                &format!(
                    "wget https://github.com/mycrl/third-party/releases/download/distributions/libyuv-{}-{}.a -O libyuv.a", 
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
