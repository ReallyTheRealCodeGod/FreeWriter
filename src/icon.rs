use std::sync::{Arc, OnceLock};

use eframe::egui::IconData;

/// The app's icon file; build.rs also puts it in writer.exe for Explorer.
/// On a Mac the app's icon comes from `assets/writer.icns` in the app bundle.
const ICO: &[u8] = include_bytes!("../assets/writer.ico");

/// The icon for the windows (title bar, taskbar, Alt+Tab): the largest image
/// in the icon file, which is stored as a PNG.
pub fn icon() -> Option<Arc<IconData>> {
    static ICON: OnceLock<Option<Arc<IconData>>> = OnceLock::new();
    ICON.get_or_init(|| {
        let png = largest_image(ICO)?;
        eframe::icon_data::from_png_bytes(png).ok().map(Arc::new)
    })
    .clone()
}

/// The bytes of the biggest image in an .ico file.
fn largest_image(ico: &[u8]) -> Option<&[u8]> {
    let count = u16::from_le_bytes(ico.get(4..6)?.try_into().ok()?) as usize;
    let entries = (0..count).filter_map(|i| ico.get(6 + 16 * i..6 + 16 * (i + 1)));
    // A width of 0 means 256.
    let largest = entries.max_by_key(|entry| if entry[0] == 0 { 256 } else { u32::from(entry[0]) })?;
    let size = u32::from_le_bytes(largest[8..12].try_into().ok()?) as usize;
    let offset = u32::from_le_bytes(largest[12..16].try_into().ok()?) as usize;
    ico.get(offset..offset.checked_add(size)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_is_the_256_pixel_image() {
        let icon = icon().expect("the icon decodes");
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }

    #[test]
    fn a_broken_icon_file_gives_nothing() {
        assert_eq!(largest_image(&[]), None);
        assert_eq!(largest_image(&[0, 0, 1, 0, 1, 0]), None);
        assert_eq!(largest_image(&ICO[..200]), None, "cut short before the image");
    }
}
