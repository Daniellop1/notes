use ratatui::style::Color;

#[derive(Clone, Copy)]
pub struct Theme {
    pub top_bg: Color,
    pub top_fg: Color,
    pub border: Color,
    pub border_focused: Color,
    pub status_normal: Color,
    pub status_insert: Color,
    pub status_command: Color,
    pub status_visual: Color,
    pub status_other: Color,
    pub text_normal: Color,
    pub text_heading: Color,
    pub text_quote: Color,
    pub text_bullet: Color,
    pub text_search: Color,
    pub text_search_bg: Color,
}

impl Theme {
    pub fn default_theme() -> Self {
        Self {
            top_bg: Color::Blue,
            top_fg: Color::Black,
            border: Color::Reset,
            border_focused: Color::Reset,
            status_normal: Color::Blue,
            status_insert: Color::Green,
            status_command: Color::Yellow,
            status_visual: Color::Rgb(255, 165, 0), // Orange
            status_other: Color::Magenta,
            text_normal: Color::White,
            text_heading: Color::LightBlue,
            text_quote: Color::Cyan,
            text_bullet: Color::Yellow,
            text_search: Color::Black,
            text_search_bg: Color::Yellow,
        }
    }

    pub fn hacker_theme() -> Self {
        Self {
            top_bg: Color::Green,
            top_fg: Color::Black,
            border: Color::DarkGray,
            border_focused: Color::Green,
            status_normal: Color::Green,
            status_insert: Color::LightGreen,
            status_command: Color::DarkGray,
            status_visual: Color::Yellow,
            status_other: Color::DarkGray,
            text_normal: Color::Green,
            text_heading: Color::LightGreen,
            text_quote: Color::DarkGray,
            text_bullet: Color::Green,
            text_search: Color::Black,
            text_search_bg: Color::LightGreen,
        }
    }

    pub fn ocean_theme() -> Self {
        Self {
            top_bg: Color::Cyan,
            top_fg: Color::Black,
            border: Color::Blue,
            border_focused: Color::Cyan,
            status_normal: Color::Cyan,
            status_insert: Color::LightCyan,
            status_command: Color::LightBlue,
            status_visual: Color::Magenta,
            status_other: Color::Blue,
            text_normal: Color::Cyan,
            text_heading: Color::LightCyan,
            text_quote: Color::Blue,
            text_bullet: Color::Blue,
            text_search: Color::Black,
            text_search_bg: Color::White,
        }
    }

    pub fn light_theme() -> Self {
        Self {
            top_bg: Color::White,
            top_fg: Color::Black,
            border: Color::Black,
            border_focused: Color::Black,
            status_normal: Color::White,
            status_insert: Color::Rgb(220, 220, 220),
            status_command: Color::Rgb(150, 150, 150),
            status_visual: Color::Yellow,
            status_other: Color::DarkGray,
            text_normal: Color::White, 
            text_heading: Color::White,
            text_quote: Color::Rgb(150, 150, 150),
            text_bullet: Color::White,
            text_search: Color::Black,
            text_search_bg: Color::White,
        }
    }
}
