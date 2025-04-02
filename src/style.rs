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
pub const text_dimmed: Color = rgb!(126, 126, 126);
pub const text_statusbar: Color = rgb!(255, 255, 255);
pub const text_sidebar_selected: Color = rgb!(204, 204, 204);

pub const text_model: Color = rgb!(231, 231, 231);
pub const text_model_primary: Color = rgb!(0, 120, 212);

pub const token_normal: Color = rgb!(240, 240, 240);
pub const token_number: Color = rgb!(181, 206, 168);
pub const token_match: Color = text;
pub const token_string: Color = rgb!(206, 145, 120);
pub const token_ml_string: Color = rgb!(215, 186, 125);
pub const token_comment: Color = rgb!(106, 153, 85);
pub const token_ml_comment: Color = rgb!(99, 142, 80);
pub const token_keyword1: Color = rgb!(86, 156, 214);
pub const token_keyword2: Color = rgb!(78, 201, 176);
pub const token_keyword3: Color = rgb!(195, 133, 190);
