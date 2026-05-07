use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::{fs, path::Path};

const CJK_FONT_NAME: &str = "system_cjk";

const WINDOWS_CJK_FONT_CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\msyh.ttc",
    r"C:\Windows\Fonts\msyh.ttf",
    r"C:\Windows\Fonts\simsun.ttc",
    r"C:\Windows\Fonts\simhei.ttf",
    r"C:\Windows\Fonts\NotoSansCJK-Regular.ttc",
];

pub fn configure(ctx: &egui::Context) {
    let Some(bytes) = load_first_available_font(WINDOWS_CJK_FONT_CANDIDATES) else {
        return;
    };

    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        CJK_FONT_NAME.to_owned(),
        std::sync::Arc::new(FontData::from_owned(bytes)),
    );

    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, CJK_FONT_NAME.to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
        family.push(CJK_FONT_NAME.to_owned());
    }

    ctx.set_fonts(fonts);
}

fn load_first_available_font(paths: &[&str]) -> Option<Vec<u8>> {
    paths
        .iter()
        .map(Path::new)
        .find_map(|path| fs::read(path).ok())
}
