#![allow(unused)]

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

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
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
                    -Uri https://github.com/mycrl/distributions/releases/download/distributions/obs-windows-x64.lib \
                    -OutFile obs.lib",
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
        .include(&join(&out_dir, "./obs-studio")?)
        .include("../common/include");

    #[cfg(target_os = "windows")]
    {
        compiler.define("WIN32", None);

        println!("cargo:rustc-link-lib=mfreadwrite");
        println!("cargo:rustc-link-lib=mfplat");
        println!("cargo:rustc-link-lib=mfuuid");
        println!("cargo:rustc-link-lib=mf");
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

#[cfg(target_os = "macos")]
fn main() {}
