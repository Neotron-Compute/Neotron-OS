#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(not(target_os = "none"))]
fn main() {
    neotron_sdk::init();
}

#[no_mangle]
extern "C" fn neotron_main() -> i32 {
    desktop::main()
}

// End of file
