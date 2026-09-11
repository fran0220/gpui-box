fn main() {
    println!("cargo:rerun-if-changed=native/host.h");
    println!("cargo:rerun-if-changed=native/host.m");
    println!("cargo:rerun-if-changed=native/geometry.h");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("ios") {
        return;
    }
    cc::Build::new()
        .file("native/host.m")
        .flag("-fobjc-arc")
        .flag("-fmodules")
        .flag("-Wall")
        .flag("-Wextra")
        .compile("gpui_ios_host");
    for framework in ["UIKit", "Foundation", "QuartzCore", "Metal"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}
