use std::fmt;
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

    pub fn is_empty(&self) -> bool {
        self.rope.is_empty()
    }
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.rope
                .iter()
                .map(|(g, _)| g.as_str())
                .collect::<String>()
        )
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
