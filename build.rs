fn main() {
    println!("cargo:rustc-link-arg-bin=flash1002=-Tneotron-flash-1000.ld");
    println!("cargo:rustc-link-arg-bin=flash0802=-Tneotron-flash-0800.ld");
    println!("cargo:rustc-link-arg-bin=flash0002=-Tneotron-flash-0000.ld");
}
