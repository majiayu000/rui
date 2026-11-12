fn main() {
    // Fluent is Slint's default style where no native (Qt) style is built in.
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/fixtures.slint", config).expect("fixtures.slint compiles");
}
