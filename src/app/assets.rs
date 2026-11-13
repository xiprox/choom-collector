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

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == "brand/mark.svg" {
            return Ok(Some(Cow::Borrowed(MARK)));
        }
        gpui_kit::assets::AllAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_kit::assets::AllAssets.list(path)
    }
}
