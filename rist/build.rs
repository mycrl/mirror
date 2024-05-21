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
        Err(anyhow!("{}", unsafe {
            String::from_utf8_unchecked(output.stderr)
        }))
    } else {
        Ok(unsafe { String::from_utf8_unchecked(output.stdout) })
    }
}

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=./build.rs");

    let out_dir = env::var("OUT_DIR")?;
    let rist_prefix = join(&out_dir, "./librist")?;
    if !is_exsit(&rist_prefix) {
        exec(
            "git clone --branch v0.2.10 https://code.videolan.org/rist/librist.git",
            &out_dir,
        )?;
    }

    if !is_exsit(&join(&rist_prefix, "./build/librist.a")?) {
        exec(&[
            "mkdir build",
            "cd build",
            "meson .. --default-library=static --buildtype=release -Db_lto=true -Dtest=false -Dbuilt_tools=false -Dbuiltin_cjson=true",
            "ninja"
        ].join(if cfg!(target_os = "windows") { "; " } else { " && " }), &rist_prefix)?;
    }

    println!("cargo:rustc-link-search=all={}/build", rist_prefix);
    println!("cargo:rustc-link-lib=rist");
    Ok(())
}
