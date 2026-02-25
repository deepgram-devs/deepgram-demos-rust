use ratatui::style::Color;

/// A complete color theme for the TUI.
pub struct Theme {
    pub name: &'static str,
    pub description: &'static str,

    /// Primary accent: text-side popups, help border, spinner, progress bar.
    pub primary: Color,
    pub primary_light: Color,

    /// Secondary accent: focused panel borders/titles, voice filter popup, info logs.
    pub secondary: Color,
    pub secondary_light: Color,

    /// Tertiary accent: audio format popup.
    pub tertiary: Color,

    /// Quaternary accent: sample rate popup.
    pub quaternary: Color,

    /// Status / feedback colors.
    pub success: Color,
    pub warning: Color,
    pub error: Color,

    /// Saved text list fade gradient (near selected → distant items).
    pub text_list_near: (u8, u8, u8),
    pub text_list_far:  (u8, u8, u8),

    /// Voice list fade gradient.
    pub voice_list_near: (u8, u8, u8),
    pub voice_list_far:  (u8, u8, u8),
}

pub const DEFAULT_THEME_INDEX: usize = 0;

pub static THEMES: [Theme; 3] = [
    // ── Deepgram ──────────────────────────────────────────────────────────────
    Theme {
        name: "Deepgram",
        description: "Deepgram brand: spring green + azure",
        primary:        Color::Rgb(19,  239, 147), // #13ef93 spring green
        primary_light:  Color::Rgb(161, 249, 212), // #a1f9d4
        secondary:      Color::Rgb(20,  154, 251), // #149afb azure
        secondary_light:Color::Rgb(161, 215, 253), // #a1d7fd
        tertiary:       Color::Rgb(238, 2,   140), // #ee028c magenta
        quaternary:     Color::Rgb(174, 99,  249), // #ae63f9 violet
        success:        Color::Rgb(18,  183, 106), // #12b76a
        warning:        Color::Rgb(254, 200, 75),  // #fec84b
        error:          Color::Rgb(240, 68,  56),  // #f04438
        text_list_near:  (161, 249, 212),
        text_list_far:   (60,  110, 80),
        voice_list_near: (161, 215, 253),
        voice_list_far:  (50,  90,  130),
    },
    // ── Nord ──────────────────────────────────────────────────────────────────
    Theme {
        name: "Nord",
        description: "Arctic, north-bluish palette by Arctic Ice Studio",
        primary:        Color::Rgb(163, 190, 140), // nord14 green
        primary_light:  Color::Rgb(197, 214, 180), // lighter nord14
        secondary:      Color::Rgb(136, 192, 208), // nord8 frost
        secondary_light:Color::Rgb(180, 218, 230), // lighter nord8
        tertiary:       Color::Rgb(180, 142, 173), // nord15 purple
        quaternary:     Color::Rgb(208, 135, 112), // nord12 orange
        success:        Color::Rgb(163, 190, 140), // nord14
        warning:        Color::Rgb(235, 203, 139), // nord13 yellow
        error:          Color::Rgb(191, 97,  106), // nord11 red
        text_list_near:  (163, 190, 140),
        text_list_far:   (55,  75,  50),
        voice_list_near: (136, 192, 208),
        voice_list_far:  (40,  70,  85),
    },
    // ── Synthwave Outrun ──────────────────────────────────────────────────────
    Theme {
        name: "Synthwave Outrun",
        description: "Neon lights, retro-futuristic vibes",
        primary:        Color::Rgb(255, 45,  125), // neon pink
        primary_light:  Color::Rgb(255, 148, 196), // light pink
        secondary:      Color::Rgb(0,   240, 255), // electric cyan
        secondary_light:Color::Rgb(128, 248, 255), // light cyan
        tertiary:       Color::Rgb(157, 0,   255), // electric purple
        quaternary:     Color::Rgb(255, 114, 0),   // neon orange
        success:        Color::Rgb(0,   255, 135), // neon green
        warning:        Color::Rgb(255, 227, 0),   // electric yellow
        error:          Color::Rgb(255, 65,  54),  // neon red
        text_list_near:  (255, 148, 196),
        text_list_far:   (100, 20,  60),
        voice_list_near: (128, 248, 255),
        voice_list_far:  (20,  70,  90),
    },
];
