//! The window's pictures: the mark, and Lucide's icons from gpui-kit.

use std::borrow::Cow;

use gpui_kit::*;

const MARK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><polygon points="2,14 5,14 9,2 6,2" fill="#000"/><polygon points="7,14 10,14 14,2 11,2" fill="#000"/></svg>"##;

/// IBM Plex, under the SIL Open Font License 1.1 (assets/fonts/OFL.txt), built into the program so
/// the window looks the same on every PC.
const FONTS: [&[u8]; 5] = [
    include_bytes!("../../assets/fonts/IBMPlexSans-Regular.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexSans-Medium.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexSans-SemiBold.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-Medium.ttf"),
];

pub fn load_fonts(cx: &App) {
    if let Err(e) = cx.text_system().add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect()) {
        // The system's fonts draw everything just as well, only not in Plex.
        log::warn!("couldn't load IBM Plex: {e:#}");
    }
}

/// The stop square everyone knows from IDEs, a solid rounded square that ends the session.
const STOP: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><rect x="2.5" y="2.5" width="11" height="11" rx="1.5" fill="#000"/></svg>"##;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == "brand/mark.svg" {
            return Ok(Some(Cow::Borrowed(MARK)));
        }
        if path == "brand/stop.svg" {
            return Ok(Some(Cow::Borrowed(STOP)));
        }
        let found = gpui_kit::assets::AllAssets.load(path)?;
        // Lucide draws with a 2-unit stroke on a 24-unit grid, which at 14px reads as a heavy web
        // icon; 1.75 draws about 1px, in line with the text's own weight (the user, 2026-11-13).
        if path.starts_with("icons/")
            && let Some(svg) = &found
        {
            let thin = String::from_utf8_lossy(svg).replace("stroke-width=\"2\"", "stroke-width=\"1.75\"");
            return Ok(Some(Cow::Owned(thin.into_bytes())));
        }
        Ok(found)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_kit::assets::AllAssets.list(path)
    }
}
