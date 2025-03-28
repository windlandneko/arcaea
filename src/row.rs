use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

type Cell = (String, usize);

pub struct Row {
    pub rope: Vec<Cell>,
}

impl Row {
    pub fn len(&self) -> usize {
        self.rope.len()
    }

    pub fn to_string(&self) -> String {
        self.rope
            .iter()
            .map(|(grapheme, _)| grapheme.as_str())
            .collect()
    }
}

impl From<String> for Row {
    fn from(string: String) -> Self {
        Self {
            rope: string
                .graphemes(true)
                .map(|g| (g.to_string(), g.width()))
                .collect(),
        }
    }
}
