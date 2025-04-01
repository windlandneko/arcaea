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

pub const background: Color = rgb!(59, 34, 76);
pub const background_selected: Color = rgb!(164, 160, 232);
pub const background_primary: Color = rgb!(40, 23, 51);
pub const text_primary: Color = rgb!(219, 191, 239);
pub const text: Color = rgb!(255, 255, 255);
pub const text_selected_whitespace: Color = rgb!(255, 255, 255);
pub const text_linenum: Color = rgb!(90, 89, 119);
pub const text_linenum_selected: Color = rgb!(219, 191, 239);
