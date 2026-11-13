//! The window's pictures: the mark, and Lucide's icons from gpui-kit.

use std::borrow::Cow;

use gpui_kit::*;

const MARK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><polygon points="2,14 5,14 9,2 6,2" fill="#000"/><polygon points="7,14 10,14 14,2 11,2" fill="#000"/></svg>"##;

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
