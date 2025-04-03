use std::fmt::Display;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::str::FromStr;

use crate::error::Error;

/// The "Highlight State" of the row
#[derive(Clone, Default, PartialEq, Eq)]
pub enum TokenState {
    /// Normal state.
    #[default]
    Normal,
    /// A multi-line comment has been open, but not yet closed.
    MultiLineComment,
    /// A string has been open with the given quote character (for instance
    /// b'\'' or b'"'), but not yet closed.
    String(String),
    /// A multi-line string has been open, but not yet closed.
    MultiLineString,
}

/// Type of syntax highlighting for a single rendered character.
///
/// Each `HLType` is associated with a color, via its discriminant. The ANSI
/// color is equal to the discriminant, modulo 100. The colors are described
/// here: <https://en.wikipedia.org/wiki/ANSI_escape_code#Colors>
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum TokenType {
    Normal,
    Number,
    Match,
    String,
    MlString,
    Comment,
    MlComment,
    Keyword1,
    Keyword2,
    Keyword3,
}

/// Configuration for syntax highlighting.
#[derive(Clone, Default)]
pub struct Syntax {
    /// The name of the language, e.g. "Rust".
    pub name: String,
    /// Whether to highlight numbers.
    pub highlight_numbers: bool,
    /// Quotes for single-line strings.
    pub sl_string_quotes: Vec<String>,
    /// The tokens that starts a single-line comment, e.g. "//".
    pub sl_comment_start: Vec<String>,
    /// The tokens that start and end a multi-line comment, e.g. ("/*", "*/").
    pub ml_comment_delims: Option<(String, String)>,
    /// The token that start and end a multi-line strings, e.g. "\"\"\"" for
    /// Python.
    pub ml_string_delim: Option<String>,
    /// Keywords to highlight and there corresponding `HLType` (typically
    /// `HLType::Keyword1` or `HLType::Keyword2`)
    pub keywords: Vec<(TokenType, Vec<String>)>,
}

/// Process an INI file.
///
/// The `kv_fn` function will be called for each key-value pair in the file.
/// Typically, this function will update a configuration instance.
pub fn process_ini_file<F>(path: &Path, kv_fn: &mut F) -> Result<(), Error>
where
    F: FnMut(&str, &str) -> Result<(), String>,
{
    let file = fs::File::open(path).map_err(|e| Error::FileError(path.into(), 0, e.to_string()))?;
    for (i, line) in BufReader::new(file).lines().enumerate() {
        let (i, line) = (i + 1, line?);
        let mut parts = line.trim_start().splitn(2, '=');
        match (parts.next(), parts.next()) {
            (Some(comment_line), _) if comment_line.starts_with(&['#', ';'][..]) => (),
            (Some(k), Some(v)) => {
                kv_fn(k.trim_end(), v).map_err(|r| Error::FileError(path.into(), i, r))?
            }
            (Some(""), None) | (None, _) => (), // Empty line
            (Some(_), None) => {
                return Err(Error::FileError(path.into(), i, String::from("No '='")))
            }
        }
    }
    Ok(())
}

/// Trim a value (right-hand side of a key=value INI line) and parses it.
pub fn pv<T: FromStr<Err = E>, E: Display>(value: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|e| format!("Parser error: {e}"))
}

/// Split a comma-separated list of values (right-hand side of a
/// key=value1,value2,... INI line) and parse it as a Vec.
pub fn pvs<T: FromStr<Err = E>, E: Display>(value: &str) -> Result<Vec<T>, String> {
    value.split(", ").map(pv).collect()
}

impl Syntax {
    /// Return the syntax configuration corresponding to the given file
    /// extension, if a matching INI file is found in a config directory.
    pub fn get(ext: &str) -> Result<Option<Self>, Error> {
        match fs::read_dir("syntax.d") {
            Ok(dir_entries) => {
                for dir_entry in dir_entries {
                    let (sc, extensions) = Self::from_file(&dir_entry?.path())?;
                    if extensions.into_iter().any(|e| e == ext) {
                        return Ok(Some(sc));
                    };
                }
            }
            Err(_) => return Ok(None),
        }

        Ok(None)
    }

    /// Load a `SyntaxConf` from file.
    pub fn from_file(path: &Path) -> Result<(Self, Vec<String>), Error> {
        let (mut sc, mut extensions) = (Self::default(), Vec::new());
        process_ini_file(path, &mut |key, val| {
            match key {
                "name" => sc.name = pv(val)?,
                "extensions" => extensions.extend(val.split(", ").map(|u| String::from(u))),
                "highlight_numbers" => sc.highlight_numbers = pv(val)?,
                "singleline_string_quotes" => sc.sl_string_quotes = pvs(val)?,
                "singleline_comment_start" => sc.sl_comment_start = pvs(val)?,
                "multiline_comment_delims" => {
                    sc.ml_comment_delims = match &val.split(", ").collect::<Vec<_>>()[..] {
                        [v1, v2] => Some((pv(v1)?, pv(v2)?)),
                        d => return Err(format!("Expected 2 delimiters, got {}", d.len())),
                    }
                }
                "multiline_string_delim" => sc.ml_string_delim = Some(pv(val)?),
                "keywords_1" => sc.keywords.push((TokenType::Keyword1, pvs(val)?)),
                "keywords_2" => sc.keywords.push((TokenType::Keyword2, pvs(val)?)),
                "keywords_3" => sc.keywords.push((TokenType::Keyword3, pvs(val)?)),
                _ => return Err(format!("Invalid key: {key}")),
            }
            Ok(())
        })?;
        Ok((sc, extensions))
    }
}

#[cfg(test)]
#[cfg(not(target_family = "wasm"))] // No filesystem on wasm
mod tests {
    use std::collections::HashSet;
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn syntax_d_files() {
        let mut file_count = 0;
        let mut syntax_names = HashSet::new();
        for path in fs::read_dir("syntax.d").unwrap() {
            let (conf, extensions) = Syntax::from_file(&path.unwrap().path()).unwrap();
            assert!(!extensions.is_empty());
            syntax_names.insert(conf.name);
            file_count += 1;
        }
        assert!(file_count > 0);
        assert_eq!(file_count, syntax_names.len());
    }

    #[test]
    fn conf_from_invalid_path() {
        let tmp_dir = TempDir::new().expect("Could not create temporary directory");
        let tmp_path = tmp_dir.path().join("path_does_not_exist.ini");
        match Syntax::from_file(&tmp_path) {
            Ok(_) => panic!("Conf::from_file should return an error"),
            Err(Error::FileError(path, 0, _)) if path == tmp_path => (),
            Err(e) => panic!("Unexpected error {:?}", e),
        }
    }
}
