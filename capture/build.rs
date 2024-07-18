use std::{env, fs, path::Path, process::Command};

use anyhow::anyhow;

fn join(root: &str, next: &str) -> anyhow::Result<String> {
    Ok(Path::new(root)
        .join(next)
        .to_str()
        .ok_or_else(|| anyhow!("Failed to path into string."))?
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
        Err(anyhow!("{}", unsafe {
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

    let target = env::var("TARGET")?;
    let out_dir = env::var("OUT_DIR")?;
    let is_debug = env::var("DEBUG")
        .map(|label| label == "true")
        .unwrap_or(true);

    #[cfg(target_os = "windows")]
    {
        if !is_exsit(&join(&out_dir, "obs.lib")?) {
            exec(
                "Invoke-WebRequest \
                    -Uri https://github.com/mycrl/mirror/releases/download/distributions/obs-windows-x64.lib \
                    -OutFile obs.lib",
                &out_dir,
            )?;
        }

        if !is_exsit(&join(&out_dir, "yuv.lib")?) {
            exec(
                "Invoke-WebRequest \
                    -Uri https://github.com/mycrl/mirror/releases/download/distributions/yuv-windows-x64.lib \
                    -OutFile yuv.lib",
                &out_dir,
            )?;
        }
    }

    if !is_exsit(&join(&out_dir, "./obs-studio")?) {
        exec(
            "git clone --branch release/30.1 https://github.com/obsproject/obs-studio",
            &out_dir,
        )?;
    }

    if !is_exsit(&join(&out_dir, "./libyuv")?) {
        exec("git clone https://github.com/lemenkov/libyuv", &out_dir)?;
    }

    let mut compiler = cc::Build::new();
    compiler
        .cpp(true)
        .std("c++20")
        .debug(is_debug)
        .static_crt(true)
        .target(&target)
        .warnings(false)
        .out_dir(&out_dir)
        .file("./lib/capture.cpp")
        .file("./lib/camera.cpp")
        .file("./lib/desktop.cpp")
        .include(&join(&out_dir, "./obs-studio")?)
        .include(&join(&out_dir, "./libyuv/include")?)
        .include("../common/include");

    #[cfg(target_os = "windows")]
    {
        compiler.define("WIN32", None);

        println!("cargo:rustc-link-lib=mfreadwrite");
        println!("cargo:rustc-link-lib=mfplat");
        println!("cargo:rustc-link-lib=mfuuid");
        println!("cargo:rustc-link-lib=mf");
        println!("cargo:rustc-link-lib=yuv");
    }

    #[cfg(target_os = "linux")]
    {
        compiler.define("LINUX", None);
    }

    compiler.compile("capture");

    println!("cargo:rustc-link-search=all={}", &out_dir);
    println!("cargo:rustc-link-lib=obs");

    Ok(())
}
