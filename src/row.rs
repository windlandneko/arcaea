use std::{fmt, iter::repeat};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{
    syntax::{TokenState, TokenType},
    Syntax,
};

type Cell = (String, usize);

#[derive(Default, Clone, PartialEq, Eq)]
pub struct Row {
    pub rope: Vec<Cell>,

    pub syntax: Vec<TokenType>,
    pub final_state: TokenState,
}

impl Row {
    pub fn len(&self) -> usize {
        self.rope.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.is_empty()
    }

    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.rope
            .iter()
            .map(|(g, _)| g.as_str())
            .collect::<String>()
    }

    /// Update the syntax highlighting types of the row.
    pub fn update_syntax(&mut self, syntax: &Syntax, state: &mut TokenState) -> TokenState {
        self.syntax.clear();

        // Delimiters for multi-line comments and multi-line strings,
        // as Option<&String, &String>
        let ml_comment_delims = syntax
            .ml_comment_delims
            .as_ref()
            .map(|(start, end)| (start, end));
        let ml_string_delims = syntax.ml_string_delim.as_ref().map(|x| (x, x));

        let str = self.to_string();
        let byte_offset = self
            .rope
            .iter()
            .scan(0, |sum, (g, _)| {
                *sum += g.len();
                Some(*sum - g.len())
            })
            .collect::<Vec<usize>>();

        /*
        // wad awdawd '"AW:D'2e12e"e" @!E: "!@e\\2e| !"
        // wad awdawd '"AW:D'2e12e"e" @!E: "!@e\\2e| !"
        wd
        adawddwawd
        // wad awdawd '"AW:D'2e12e"e" @!E: "!@e\\2e| !"
        awdd */

        'syntax_loop: while self.syntax.len() < self.len() {
            let i = self.syntax.len();
            let find_str = |s: &str| str[byte_offset[i]..].starts_with(s);

            if *state == TokenState::Normal && syntax.sl_comment_start.iter().any(|s| find_str(s)) {
                self.syntax
                    .extend(repeat(TokenType::Comment).take(self.len() - i));
                continue;
            }

            // Multi-line strings and multi-line comments have the same behavior; the only
            // differences are: the start/end delimiters, the `HLState`, the `HLType`.
            for (delims, mstate, mtype) in &[
                (
                    ml_comment_delims,
                    TokenState::MultiLineComment,
                    TokenType::MlComment,
                ),
                (
                    ml_string_delims,
                    TokenState::MultiLineString,
                    TokenType::MlString,
                ),
            ] {
                if let Some((start, end)) = delims {
                    if *state == *mstate {
                        if find_str(end) {
                            // Highlight the remaining symbols of the multi line comment end
                            self.syntax.extend(repeat(mtype).take(end.len()));
                            *state = TokenState::Normal;
                        } else {
                            self.syntax.push(*mtype);
                        }
                        continue 'syntax_loop;
                    } else if *state == TokenState::Normal && find_str(start) {
                        // Highlight the remaining symbols of the multi line comment start
                        self.syntax.extend(repeat(mtype).take(start.len()));
                        *state = mstate.clone();
                        continue 'syntax_loop;
                    }
                }
            }

            let c = &self.rope[i].0;

            // At this point, hl_state is Normal or String
            if let TokenState::String(ref quote) = *state {
                self.syntax.push(TokenType::String);
                if c == quote {
                    *state = TokenState::Normal;
                } else if *c == '\\'.to_string() && i != self.len() - 1 {
                    self.syntax.push(TokenType::String);
                }
                continue;
            } else if syntax.sl_string_quotes.contains(c) {
                *state = TokenState::String(c.clone());
                self.syntax.push(TokenType::String);
                continue;
            }

            let prev_sep = (i == 0) || is_sep(self.rope[i - 1].0.as_str());

            if syntax.highlight_numbers
                && ((c.len() == 1
                    && c.chars().next().is_some_and(|c| c.is_ascii_digit())
                    && prev_sep)
                    || (i != 0
                        && self.syntax[i - 1] == TokenType::Number
                        && !prev_sep
                        && !is_sep(c.as_str())))
            {
                self.syntax.push(TokenType::Number);
                continue;
            }

            if prev_sep {
                // This filters makes sure that names such as "in_comment" are not partially
                // highlighted (even though "in" is a keyword in rust)
                // The argument is the keyword that is matched at `i`.
                let s_filter = |kw: &str| {
                    self.rope
                        .get(i + kw.len())
                        .map_or(true, |c| is_sep(c.0.as_str()))
                };
                for (keyword_highlight_type, kws) in &syntax.keywords {
                    for keyword in kws.iter().filter(|kw| find_str(kw) && s_filter(kw)) {
                        self.syntax
                            .extend(repeat(*keyword_highlight_type).take(keyword.len()));
                    }
                }
            }

            self.syntax.push(TokenType::Normal);
        }

        // String state doesn't propagate to the next row
        self.final_state = if matches!(state, TokenState::String(_)) {
            TokenState::Normal
        } else {
            state.clone()
        };
        self.final_state.clone()
    }
}

/// Return whether `c` is an ASCII separator.
fn is_sep(c: &str) -> bool {
    c.len() == 1
        && c.chars().next().is_some_and(|c| {
            c.is_ascii_whitespace() || c == '\0' || (c.is_ascii_punctuation() && c != '_')
        })
}

impl From<&str> for Row {
    fn from(string: &str) -> Self {
        let rope: Vec<Cell> = string
            .graphemes(true)
            .map(|g| (g.to_string(), g.width()))
            .collect();
        Self {
            syntax: vec![],
            final_state: TokenState::Normal,
            rope,
        }
    }
}

impl From<Vec<Cell>> for Row {
    fn from(rope: Vec<Cell>) -> Self {
        Self {
            syntax: vec![],
            final_state: TokenState::Normal,
            rope,
        }
    }
}

impl fmt::Debug for Row {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Row(\"{}\")", self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{syntax::TokenState, Error};

    #[test]
    fn test_row() {
        let row = Row::from("Hello, world!");
        assert_eq!(row.len(), 13);
        assert_eq!(row.to_string(), "Hello, world!");
        assert_eq!(row.is_empty(), false);
    }

    #[test]
    fn test_update_syntax() -> Result<(), Error> {
        let mut row = Row::from("let x = 42;");
        let syntax = Syntax::get("js")?.unwrap();
        let mut state = TokenState::Normal;
        row.update_syntax(&syntax, &mut state);
        assert_eq!(row.syntax.len(), 11);

        assert_eq!(row.syntax[0], TokenType::Keyword1);
        assert_eq!(row.syntax[1], TokenType::Normal);
        // assert_eq!(row.syntax[2], TokenType::Operator);
        // assert_eq!(row.syntax[3], TokenType::Identifier);
        assert_eq!(row.syntax[4], TokenType::Normal);
        assert_eq!(row.syntax[5], TokenType::Number);
        assert_eq!(row.syntax[6], TokenType::Normal);
        // assert_eq!(row.syntax[7], TokenType::Operator);
        // assert_eq!(row.syntax[8], TokenType::Identifier);
        assert_eq!(row.syntax[9], TokenType::Normal);
        // assert_eq!(row.syntax[10], TokenType::End);

        Ok(())
    }
}
