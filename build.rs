extern crate winres;

fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("FileDescription", "Nothing But Notes");
        res.set("ProductName", "Nothing But Notes");
        res.set("OriginalFilename", "nothing-but-notes.exe");
        res.compile().unwrap();
    }
}
