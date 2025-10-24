fn main() {
    // Essential linker script for ESP32
    println!("cargo:rustc-link-arg=-Tlinkall.x");

    // Set up helpful error handling for common ESP32 issues
    setup_error_handler();
}

fn setup_error_handler() {
    println!(
        "cargo:rustc-link-arg=-Wl,--error-handling-script={}",
        std::env::current_exe().unwrap().display()
    );
}
