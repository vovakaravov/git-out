use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Hsla, px, rgb, rgba};

/// Kanagawa Dragon charcoal with a whisper-subtle pine tint & sage-emerald accent.
pub mod gh_colors {
    pub const CANVAS_DEFAULT: u32 = 0x161817;
    pub const CANVAS_SUBTLE: u32 = 0x1b1e1c;
    pub const CANVAS_INSET: u32 = 0x0c0e0d;
    pub const CANVAS_OVERLAY: u32 = 0x242826;

    pub const BTN_BG: u32 = 0x333835;
    pub const BTN_HOVER: u32 = 0x404642;
    pub const BTN_ACTIVE: u32 = 0x2b302d;

    pub const BORDER: u32 = 0x363c38;
    pub const BORDER_MUTED: u32 = 0x262b28;

    pub const FG: u32 = 0xdcd7ba;
    pub const FG_MUTED: u32 = 0x7e837c;
    pub const FG_SUBTLE: u32 = 0x9e9b93;

    pub const ACCENT: u32 = 0x7eb88a;
    pub const ACCENT_EMPHASIS: u32 = 0x4b8f62;

    pub const BLUE: u32 = 0x7fb4ca;
    pub const BLUE_EMPHASIS: u32 = 0x5c8fa8;

    pub const GREEN_BTN: u32 = 0x3f8458;
    pub const GREEN_BTN_HOVER: u32 = 0x4e9969;
    pub const GREEN_BTN_ACTIVE: u32 = 0x35724b;
    pub const GREEN_FG: u32 = 0x7eb88a;

    pub const RED_BTN: u32 = 0xb85955;
    pub const RED_BTN_HOVER: u32 = 0xc96b67;
    pub const RED_FG: u32 = 0xd47772;

    pub const YELLOW: u32 = 0xc4b28a;
    pub const PURPLE: u32 = 0x957fb8;
    pub const ORANGE: u32 = 0xc89574;
    pub const PINK: u32 = 0xb58ea8;
    pub const CYAN: u32 = 0x7aa89f;

    /// Subway rail colors for the commit graph lanes.
    pub const RAIL_COLORS: [u32; 8] = [
        GREEN_FG, BLUE, PURPLE, ORANGE, PINK, CYAN, YELLOW, RED_FG,
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
        c.danger_active = hex_color(0x9c4845);
        c.danger_foreground = hex_color(0xffffff);

        c.button_danger = hex_color(gh_colors::RED_BTN);
        c.button_danger_hover = hex_color(gh_colors::RED_BTN_HOVER);
        c.button_danger_active = hex_color(0x9c4845);
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
        c.info_hover = hex_color(0x72a5bd);
        c.info_active = hex_color(0x4d7c94);
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
        c.ring = hex_color(gh_colors::ACCENT);
        c.caret = hex_color(gh_colors::ACCENT);
        c.selection = hex_alpha(0x4b8f624d);

        c.link = hex_color(gh_colors::ACCENT);
        c.link_hover = hex_color(0x98cfa3);
        c.link_active = hex_color(gh_colors::ACCENT_EMPHASIS);

        c.list = hex_color(gh_colors::CANVAS_DEFAULT);
        c.list_even = hex_color(gh_colors::CANVAS_DEFAULT);
        c.list_head = hex_color(gh_colors::CANVAS_SUBTLE);
        c.list_hover = hex_color(gh_colors::CANVAS_SUBTLE);
        c.list_active = hex_alpha(0x4b8f622a);
        c.list_active_border = hex_color(gh_colors::ACCENT_EMPHASIS);
        c.list_highlight_foreground = hex_color(gh_colors::ACCENT);

        c.tab_bar = hex_color(gh_colors::CANVAS_INSET);
        c.tab_bar_segmented = hex_color(gh_colors::CANVAS_SUBTLE);
        c.tab = gpui_kit::transparent_black();
        c.tab_foreground = hex_color(gh_colors::FG_MUTED);
        c.tab_active = hex_color(gh_colors::BTN_BG);
        c.tab_active_foreground = hex_color(gh_colors::FG);

        c.scrollbar = hex_alpha(0x00000000);
        c.scrollbar_thumb = hex_alpha(0x7e837c66);
        c.scrollbar_thumb_hover = hex_alpha(0x9e9b93aa);

        c.red = hex_color(gh_colors::RED_FG);
        c.green = hex_color(gh_colors::GREEN_FG);
        c.blue = hex_color(gh_colors::BLUE);
        c.yellow = hex_color(gh_colors::YELLOW);
        c.magenta = hex_color(gh_colors::PURPLE);
        c.cyan = hex_color(gh_colors::CYAN);
    });
}
