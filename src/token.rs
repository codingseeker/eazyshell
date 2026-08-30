#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    Word,
    Assign,
    Var,
    VarBrace,
    Arith,
    GroupOpen,
    GroupClose,
    Pipe,
    And,
    Or,
    Semicolon,
    Background,
    RedirectIn,
    RedirectOut,
    RedirectAppend,
    RedirectClobber,
    RedirectBothOut,
    RedirectHeredoc,
    RedirectHeredocTab,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub type_: TokenType,
    pub value: Option<String>,
    pub quoted: bool,
}

impl Token {
    pub fn text(&self) -> &str {
        self.value.as_deref().unwrap_or("")
    }
}
