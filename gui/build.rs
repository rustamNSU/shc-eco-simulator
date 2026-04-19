fn main() {
    slint_build::compile("ui/main.slint").expect("failed to compile Slint UI");

    #[cfg(target_os = "windows")]
    winresource::WindowsResource::new()
        .set_icon("assets/concept06_tax.ico")
        .compile()
        .expect("failed to embed Windows executable icon");
}
