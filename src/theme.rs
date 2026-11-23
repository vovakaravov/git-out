use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Hsla, px, rgb, rgba};

/// GitHub Primer Dark Default color palette.
pub mod gh_colors {
    pub const CANVAS_DEFAULT: u32 = 0x0d1117;
    pub const CANVAS_SUBTLE: u32 = 0x161b22;
    pub const CANVAS_INSET: u32 = 0x010409;
    pub const CANVAS_OVERLAY: u32 = 0x161b22;

    pub const BTN_BG: u32 = 0x21262d;
    pub const BTN_HOVER: u32 = 0x30363d;
    pub const BTN_ACTIVE: u32 = 0x282e33;

    pub const BORDER: u32 = 0x30363d;
    pub const BORDER_MUTED: u32 = 0x21262d;

    pub const FG: u32 = 0xe6edf3;
    pub const FG_MUTED: u32 = 0x8b949e;
    pub const FG_SUBTLE: u32 = 0x6e7681;

    pub const BLUE: u32 = 0x58a6ff;
    pub const BLUE_EMPHASIS: u32 = 0x1f6feb;

    pub const GREEN_BTN: u32 = 0x238636;
    pub const GREEN_BTN_HOVER: u32 = 0x2ea043;
    pub const GREEN_BTN_ACTIVE: u32 = 0x29903b;
    pub const GREEN_FG: u32 = 0x3fb950;

    pub const RED_BTN: u32 = 0xda3633;
    pub const RED_BTN_HOVER: u32 = 0xf85149;
    pub const RED_FG: u32 = 0xf85149;

    pub const YELLOW: u32 = 0xd29922;
    pub const PURPLE: u32 = 0xa371f7;
    pub const ORANGE: u32 = 0xf0883e;
    pub const PINK: u32 = 0xf778ba;
    pub const CYAN: u32 = 0x39c5cf;

    /// Subway rail colors for the commit graph lanes (GitHub Primer accents).
    pub const RAIL_COLORS: [u32; 8] = [
        BLUE, GREEN_FG, PURPLE, ORANGE, PINK, CYAN, YELLOW, RED_FG,
    ];
}

#[inline]
pub fn hex_color(hex: u32) -> Hsla {
    rgb(hex).into()
}

#[inline]
pub fn hex_alpha(hex_rgba: u32) -> Hsla {
    rgba(hex_rgba).into()
}

/// Returns the rail color for a lane color index.
#[inline]
pub fn rail_color(color_idx: usize) -> Hsla {
    hex_color(gh_colors::RAIL_COLORS[color_idx % gh_colors::RAIL_COLORS.len()])
}

/// Parses a GitHub API label hex string (e.g. `"d73a4a"` or `"#1f6feb"`).
pub fn parse_gh_label_hex(hex: &str) -> Hsla {
    let cleaned = hex.trim().trim_start_matches('#');
    if cleaned.len() == 6
        && let Ok(val) = u32::from_str_radix(cleaned, 16)
    {
        return hex_color(val);
    }
    hex_color(gh_colors::BLUE)
}

/// Returns a crisp high-contrast foreground color (light or dark) for a given hex label background.
pub fn contrast_fg_for_hex(hex: &str) -> Hsla {
    let cleaned = hex.trim().trim_start_matches('#');
    if cleaned.len() == 6
        && let Ok(val) = u32::from_str_radix(cleaned, 16)
    {
        let r = ((val >> 16) & 0xff) as f32;
        let g = ((val >> 8) & 0xff) as f32;
        let b = (val & 0xff) as f32;
        let luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
        if luminance > 0.6 {
            return hex_color(gh_colors::CANVAS_INSET);
        }
    }
    hex_color(gh_colors::FG)
}

/// Applies the GitHub Dark Default palette to gpui-component's global Theme.
pub fn apply_github_dark_theme(cx: &mut App) {
    // Switch to Dark mode first so defaults are loaded before customizing colors.
    Theme::change(ThemeMode::Dark, None, cx);

    Theme::update(cx, |theme| {
        theme.radius = px(6.);
        theme.radius_lg = px(8.);
        theme.font_size = px(13.);
        theme.mono_font_size = px(12.);

        let c = &mut theme.colors;
        c.background = hex_color(gh_colors::CANVAS_DEFAULT);
        c.foreground = hex_color(gh_colors::FG);
        c.border = hex_color(gh_colors::BORDER);
        c.window_border = hex_color(gh_colors::BORDER);
        c.muted = hex_color(gh_colors::CANVAS_SUBTLE);
        c.muted_foreground = hex_color(gh_colors::FG_MUTED);
        c.accent = hex_color(gh_colors::BTN_BG);
        c.accent_foreground = hex_color(gh_colors::FG);

        c.popover = hex_color(gh_colors::CANVAS_OVERLAY);
        c.popover_foreground = hex_color(gh_colors::FG);
        c.group_box = hex_color(gh_colors::CANVAS_SUBTLE);
        c.group_box_foreground = hex_color(gh_colors::FG);

        // GitHub-style buttons: default/secondary = slate #21262d, primary = GitHub green #238636
        c.button = hex_color(gh_colors::BTN_BG);
        c.button_hover = hex_color(gh_colors::BTN_HOVER);
        c.button_active = hex_color(gh_colors::BTN_ACTIVE);
        c.button_foreground = hex_color(gh_colors::FG);

        c.button_secondary = hex_color(gh_colors::BTN_BG);
        c.button_secondary_hover = hex_color(gh_colors::BTN_HOVER);
        c.button_secondary_active = hex_color(gh_colors::BTN_ACTIVE);
        c.button_secondary_foreground = hex_color(gh_colors::FG);

        c.primary = hex_color(gh_colors::GREEN_BTN);
        c.primary_hover = hex_color(gh_colors::GREEN_BTN_HOVER);
        c.primary_active = hex_color(gh_colors::GREEN_BTN_ACTIVE);
        c.primary_foreground = hex_color(0xffffff);

        c.button_primary = hex_color(gh_colors::GREEN_BTN);
        c.button_primary_hover = hex_color(gh_colors::GREEN_BTN_HOVER);
        c.button_primary_active = hex_color(gh_colors::GREEN_BTN_ACTIVE);
        c.button_primary_foreground = hex_color(0xffffff);

        c.secondary = hex_color(gh_colors::BTN_BG);
        c.secondary_hover = hex_color(gh_colors::BTN_HOVER);
        c.secondary_active = hex_color(gh_colors::BTN_ACTIVE);
        c.secondary_foreground = hex_color(gh_colors::FG);

        c.danger = hex_color(gh_colors::RED_BTN);
        c.danger_hover = hex_color(gh_colors::RED_BTN_HOVER);
        c.danger_active = hex_color(0xb62324);
        c.danger_foreground = hex_color(0xffffff);

        c.button_danger = hex_color(gh_colors::RED_BTN);
        c.button_danger_hover = hex_color(gh_colors::RED_BTN_HOVER);
        c.button_danger_active = hex_color(0xb62324);
        c.button_danger_foreground = hex_color(0xffffff);

        c.success = hex_color(gh_colors::GREEN_BTN);
        c.success_hover = hex_color(gh_colors::GREEN_BTN_HOVER);
        c.success_active = hex_color(gh_colors::GREEN_BTN_ACTIVE);
        c.success_foreground = hex_color(0xffffff);
        c.button_success = hex_color(gh_colors::GREEN_BTN);
        c.button_success_hover = hex_color(gh_colors::GREEN_BTN_HOVER);
        c.button_success_active = hex_color(gh_colors::GREEN_BTN_ACTIVE);
        c.button_success_foreground = hex_color(0xffffff);

        c.info = hex_color(gh_colors::BLUE_EMPHASIS);
        c.info_hover = hex_color(0x388bfd);
        c.info_active = hex_color(0x1158c7);
        c.info_foreground = hex_color(0xffffff);

        c.warning = hex_color(gh_colors::YELLOW);
        c.warning_foreground = hex_color(gh_colors::CANVAS_INSET);

        c.sidebar = hex_color(gh_colors::CANVAS_SUBTLE);
        c.sidebar_border = hex_color(gh_colors::BORDER);
        c.sidebar_foreground = hex_color(gh_colors::FG);
        c.sidebar_accent = hex_color(gh_colors::BTN_BG);
        c.sidebar_accent_foreground = hex_color(gh_colors::FG);

        c.title_bar = hex_color(gh_colors::CANVAS_INSET);
        c.title_bar_border = hex_color(gh_colors::BORDER);
        c.status_bar = hex_color(gh_colors::CANVAS_INSET);
        c.status_bar_border = hex_color(gh_colors::BORDER);

        c.input = hex_color(gh_colors::BORDER);
        c.ring = hex_color(gh_colors::BLUE);
        c.caret = hex_color(gh_colors::BLUE);
        c.selection = hex_alpha(0x1f6feb4d);

        c.link = hex_color(gh_colors::BLUE);
        c.link_hover = hex_color(0x79c0ff);
        c.link_active = hex_color(gh_colors::BLUE_EMPHASIS);

        c.list = hex_color(gh_colors::CANVAS_DEFAULT);
        c.list_even = hex_color(gh_colors::CANVAS_DEFAULT);
        c.list_head = hex_color(gh_colors::CANVAS_SUBTLE);
        c.list_hover = hex_color(gh_colors::CANVAS_SUBTLE);
        c.list_active = hex_alpha(0x1f6feb26);
        c.list_active_border = hex_color(gh_colors::BLUE_EMPHASIS);
        c.list_highlight_foreground = hex_color(gh_colors::BLUE);

        c.tab_bar = hex_color(gh_colors::CANVAS_INSET);
        c.tab_bar_segmented = hex_color(gh_colors::CANVAS_SUBTLE);
        c.tab = gpui_kit::transparent_black();
        c.tab_foreground = hex_color(gh_colors::FG_MUTED);
        c.tab_active = hex_color(gh_colors::BTN_BG);
        c.tab_active_foreground = hex_color(gh_colors::FG);

        c.scrollbar = hex_alpha(0x00000000);
        c.scrollbar_thumb = hex_alpha(0x6e768180);
        c.scrollbar_thumb_hover = hex_alpha(0x8b949ecc);

        c.red = hex_color(gh_colors::RED_FG);
        c.green = hex_color(gh_colors::GREEN_FG);
        c.blue = hex_color(gh_colors::BLUE);
        c.yellow = hex_color(gh_colors::YELLOW);
        c.magenta = hex_color(gh_colors::PURPLE);
        c.cyan = hex_color(gh_colors::CYAN);
    });
}
