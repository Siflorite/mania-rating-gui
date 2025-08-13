fn main() {
    slint_build::compile("ui/main_window.slint").unwrap();
    if cfg!(target_os = "windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("ui/icon.ico");
        res.set_manifest_file("manifest.xml");
        res.compile().unwrap();
    }
}
