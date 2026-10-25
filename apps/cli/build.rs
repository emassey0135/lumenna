//! `lum`'s main thread gets the 8 MiB stack Linux and macOS give it. Windows gives 1 MiB, which
//! clap's derived parser for every command outgrows in an unoptimised build.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bin=lum=/STACK:8388608");
    }
}
