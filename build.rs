extern crate winres;

fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("FileDescription", "Nothing But Notes — Minimal Desktop Companion");
        res.set("ProductName", "Nothing But Notes");
        res.set("CompanyName", "Nothing But Notes");
        res.set("LegalCopyright", "Copyright (c) 2026. Open source under MIT License.");
        res.set("OriginalFilename", "nothing-but-notes.exe");
        res.set("ProductVersion", "0.1.0");
        res.set("FileVersion", "0.1.0.0");
        res.compile().unwrap();
    }
}
