use std::fmt;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

type Cell = (String, usize);

#[derive(Default)]
pub struct Row(pub Vec<Cell>);

impl Row {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0.iter().map(|(g, _)| g.as_str()).collect::<String>()
        )
    }
}

impl From<String> for Row {
    fn from(string: String) -> Self {
        Self(
            string
                .graphemes(true)
                .map(|g| (g.to_string(), g.width()))
                .collect(),
        )
    }
}
