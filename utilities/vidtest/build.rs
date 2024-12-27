fn main() {
    println!("cargo:rustc-link-arg-bin=vidtest=-Tneotron-cortex-m.ld");
}
