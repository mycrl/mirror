use std::{env, fs, path::Path, process::Command};

use anyhow::Result;

fn is_exsit(dir: &str) -> bool {
    fs::metadata(dir).is_ok()
}

fn join(root: &str, next: &str) -> String {
    Path::new(root).join(next).to_str().unwrap().to_string()
}

fn exec(command: &str, work_dir: &str) -> Result<()> {
    let _ = Command::new(if cfg!(windows) { "powershell" } else { "bash" })
        .arg(if cfg!(windows) { "-command" } else { "-c" })
        .arg(command)
        .current_dir(work_dir)
        .status()?;
    Ok(())
}

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=./core/src");
    println!("cargo:rerun-if-changed=./build.rs");

    let target = env::var("TARGET")?;
    let out_dir = env::var("OUT_DIR")?;
    let is_debug = env::var("DEBUG")
        .map(|label| label == "true")
        .unwrap_or(true);

    let jthread_prefix = join(&out_dir, "./jthread");
    if !is_exsit(&jthread_prefix) {
        exec(
            "git clone --branch v1.3.3 https://github.com/j0r1/JThread jthread",
            &out_dir,
        )?;
    }

    if !is_exsit(&join(&jthread_prefix, "./src/libjthread.a")) {
        exec("cmake .", &jthread_prefix)?;
        exec("cmake --build .", &jthread_prefix)?;
    }

    if !is_exsit(&join(&jthread_prefix, "./jthread")) {
        exec("cp -r ./src ./jthread", &jthread_prefix)?;
    }

    let jrtp_prefix = join(&out_dir, "./jrtplib");
    if !is_exsit(&jrtp_prefix) {
        exec(
            "git clone --branch v3.11.2 https://github.com/j0r1/JRTPLIB jrtplib",
            &out_dir,
        )?;
    }

    if !is_exsit(&join(&jthread_prefix, "./src/libjrtp.a")) {
        exec("cmake .", &jrtp_prefix)?;
        exec("cmake --build .", &jrtp_prefix)?;
    }

    if target.find("android").is_some() {
        if !is_exsit(&join(&out_dir, "librtp.a")) {
            exec(
                "wget \
                -O libjrtp.a \
                https://github.com/mycrl/mirror/releases/download/distributions/libjrtp-arm64-v8a.a",
                &out_dir,
            )?;
        }
    } else {
        println!(
            "cargo:rustc-link-search=all={}",
            &join(&jthread_prefix, "./src")
        );

        println!(
            "cargo:rustc-link-search=all={}",
            &join(&jrtp_prefix, "./src")
        );

        println!("cargo:rustc-link-lib=jthread");
    }

    cc::Build::new()
        .cpp(true)
        .std("c++20")
        .debug(is_debug)
        .static_crt(true)
        .target(&target)
        .warnings(false)
        .out_dir(&out_dir)
        .file("./core/src/rtp.cpp")
        .includes(&[&join(&jrtp_prefix, "./src"), &jthread_prefix])
        .compile("rtp");

    println!("cargo:rustc-link-search=all={}", out_dir);
    println!("cargo:rustc-link-lib=jrtp");
    println!("cargo:rustc-link-lib=rtp");

    Ok(())
}
