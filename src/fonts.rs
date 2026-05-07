use iced::{Font, Settings};
use std::{borrow::Cow, fs, path::Path};

struct FontCandidate {
    path: &'static str,
    family_name: &'static str,
}

const WINDOWS_CJK_FONT_CANDIDATES: &[FontCandidate] = &[
    FontCandidate {
        path: r"C:\Windows\Fonts\msyh.ttc",
        family_name: "Microsoft YaHei",
    },
    FontCandidate {
        path: r"C:\Windows\Fonts\msyh.ttf",
        family_name: "Microsoft YaHei",
    },
    FontCandidate {
        path: r"C:\Windows\Fonts\simsun.ttc",
        family_name: "SimSun",
    },
    FontCandidate {
        path: r"C:\Windows\Fonts\simhei.ttf",
        family_name: "SimHei",
    },
    FontCandidate {
        path: r"C:\Windows\Fonts\NotoSansCJK-Regular.ttc",
        family_name: "Noto Sans CJK SC",
    },
];

pub fn settings() -> Settings {
    let Some((bytes, family_name)) = load_first_available_font(WINDOWS_CJK_FONT_CANDIDATES) else {
        return Settings::default();
    };

    Settings {
        fonts: vec![Cow::Owned(bytes)],
        default_font: Font::with_name(family_name),
        ..Settings::default()
    }
}

fn load_first_available_font(candidates: &[FontCandidate]) -> Option<(Vec<u8>, &'static str)> {
    candidates.iter().find_map(|candidate| {
        fs::read(Path::new(candidate.path))
            .ok()
            .map(|bytes| (bytes, candidate.family_name))
    })
}
