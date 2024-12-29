fn main() {
    println!("cargo:rustc-link-arg-bin=snake=-Tneotron-cortex-m.ld");
}
