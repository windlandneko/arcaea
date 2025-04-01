use std::fmt;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

type Cell = (String, usize);

#[derive(Default, Clone, PartialEq, Eq)]
pub struct Row(pub Vec<Cell>);

impl Row {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.0.iter().map(|(g, _)| g.as_str()).collect::<String>()
    }
}

impl From<&str> for Row {
    fn from(string: &str) -> Self {
        Self(
            string
                .graphemes(true)
                .map(|g| (g.to_string(), g.width()))
                .collect(),
        )
    }
}

impl fmt::Debug for Row {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Row(\"{}\")", self.to_string())
    }
}
