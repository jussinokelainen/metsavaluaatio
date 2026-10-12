pub fn build_maps_kml() -> String {
    String::from("foo")
}

pub fn open_browser(url: &str) {
    let mut command = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else {
        std::process::Command::new("xdg-open")
    };
    match command.arg(url).spawn() {
        Ok(_) => {}
        Err(_) => {
            println!("Failed to open browser automatically, url for manual opening:");
            println!("{}", url);
        }
    }
}
