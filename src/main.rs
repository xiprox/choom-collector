fn main() {
    // Velopack's install, update and uninstall hooks run here and may exit the process,
    // so this comes before anything else.
    velopack::VelopackApp::build().run();

    println!("CP77 Co-op Tools {}", env!("CARGO_PKG_VERSION"));
}
