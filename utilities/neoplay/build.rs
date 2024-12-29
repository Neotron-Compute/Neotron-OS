fn main() {
    println!("cargo:rustc-link-arg-bin=neoplay=-Tneotron-cortex-m.ld");
}
