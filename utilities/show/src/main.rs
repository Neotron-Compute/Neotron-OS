#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(not(target_os = "none"))]
fn main() {
    neotron_sdk::init();
}

static mut APP: show::App = show::App::new();

#[no_mangle]
extern "C" fn neotron_main() -> i32 {
    unsafe { APP.play() }
    0
}
