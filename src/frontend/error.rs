#[derive(Clone, Debug, PartialEq)]
pub struct ParserError {
    msg: String,
    pub line: usize,
    pub col: usize,
}

impl ParserError {
    pub fn new(msg: &str, line: usize, col: usize) -> Self {
        ParserError {
            msg: String::from(msg),
            line,
            col,
        }
    }

    pub fn format(&self, filename: &str) -> String {
        format!(
            "{}:{}:{} error: {}",
            filename, self.line, self.col, self.msg
        )
    }
}
