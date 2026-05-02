fn main() {
    // Ne compiler les .slint que si la feature "ui" est activée.
    if std::env::var("CARGO_FEATURE_UI").is_ok() {
        slint_build::compile("src/ui/app_window.slint").unwrap();
    }
}
