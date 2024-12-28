fn main() {
    println!("cargo:rustc-link-arg-bin=desktop=-Tneotron-cortex-m.ld");
}
