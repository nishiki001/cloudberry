fn main() {
    // A fixed Slint style: the look must not depend on the platform or on SLINT_STYLE. (No Qt
    // backend is built either: winit + the software renderer.)
    let mut config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    // Debug builds carry element ids so the interaction tests can look elements up.
    if std::env::var("PROFILE").is_ok_and(|p| p == "debug") {
        config = config.with_debug_info(true);
    }
    slint_build::compile_with_config("ui/app.slint", config).expect("slint compile");
}
