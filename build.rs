// Puts the app's icon into the Windows program as resource 1, which Explorer shows for the file and
// gpui loads for the window and the taskbar. tools/make-icon.ps1 draws it.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon/app.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon/app.ico");
        resource.compile().expect("the icon should go into the Windows program");
    }
}
