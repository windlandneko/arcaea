#![allow(non_upper_case_globals)]
use crossterm::style::Color;

macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        Color::Rgb {
            r: $r,
            g: $g,
            b: $b,
        }
    };
}

pub const background: Color = rgb!(34, 34, 34);
pub const background_selected: Color = rgb!(38, 79, 120);
pub const background_primary: Color = rgb!(166, 226, 46);
pub const background_sidebar: Color = rgb!(51, 51, 51);
pub const text_primary: Color = rgb!(34, 34, 34);
pub const text: Color = rgb!(204, 204, 204);
pub const text_selected_whitespace: Color = rgb!(255, 255, 255);
pub const text_sidebar: Color = rgb!(126, 126, 126);
pub const text_statusbar: Color = rgb!(255, 255, 255);
pub const text_sidebar_selected: Color = rgb!(204, 204, 204);
