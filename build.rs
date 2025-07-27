fn main() {
    slint_build::compile("ui/main_window.slint").unwrap();
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("ui/icon.ico");
        res.compile().unwrap();
    }
}
