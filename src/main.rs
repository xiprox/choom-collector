fn main() {
    // Velopack's install, update and uninstall hooks run here and may exit the process,
    // so this comes before anything else.
    velopack::VelopackApp::build().run();

    println!("choomlink {}", env!("CARGO_PKG_VERSION"));
}
