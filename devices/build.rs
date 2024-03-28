use std::{env, fs, path::Path, process::Command};

use anyhow::anyhow;

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
                -Uri https://github.com/mycrl/mirror/releases/download/distributions/obs.lib \
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

    cc::Build::new()
        .cpp(false)
        .std("c17")
        .debug(is_debug)
        .static_crt(true)
        .target(&target)
        .warnings(false)
        .out_dir(&out_dir)
        .file("./lib/devices.c")
        .include(&join(&out_dir, "./obs-studio")?)
        .compile("devices");

    println!("cargo:rustc-link-search=all={}", &out_dir);
    println!("cargo:rustc-link-lib=obs");
    Ok(())
}
