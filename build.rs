fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("openbsd") {
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic-symbol=priv_open_device");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
