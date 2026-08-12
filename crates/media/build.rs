#![allow(clippy::disallowed_methods, reason = "build scripts are exempt")]
fn main() {
    use std::{env, path::PathBuf, process::Command};

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    println!("cargo:rerun-if-changed=src/player/macos.m");
    cc::Build::new()
        .file("src/player/macos.m")
        .flag("-fobjc-arc")
        .compile("gpui_box_media_player");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=AVFoundation");
    println!("cargo:rustc-link-lib=framework=QuartzCore");

    let sdk_path = String::from_utf8(
        Command::new("xcrun")
            .args(["--sdk", "macosx", "--show-sdk-path"])
            .output()
            .expect("failed to query the macOS SDK path with xcrun")
            .stdout,
    )
    .expect("xcrun returned a non-UTF-8 macOS SDK path");
    let sdk_path = sdk_path.trim_end();

    println!("cargo:rerun-if-changed=src/bindings.h");
    let bindings = bindgen::Builder::default()
        .header("src/bindings.h")
        .clang_arg(format!("-isysroot{}", sdk_path))
        .clang_arg("-xobjective-c")
        .allowlist_type("CMItemIndex")
        .allowlist_type("CMSampleTimingInfo")
        .allowlist_type("CMVideoCodecType")
        .allowlist_type("VTEncodeInfoFlags")
        .allowlist_function("CMTimeMake")
        .allowlist_var("kCVPixelFormatType_.*")
        .allowlist_var("kCVReturn.*")
        .allowlist_var("VTEncodeInfoFlags_.*")
        .allowlist_var("kCMVideoCodecType_.*")
        .allowlist_var("kCMTime.*")
        .allowlist_var("kCMSampleAttachmentKey_.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .layout_tests(false)
        .generate()
        .expect("unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").expect("Cargo did not provide OUT_DIR"));
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("couldn't write dispatch bindings");
}
