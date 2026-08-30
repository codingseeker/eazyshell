use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellError {
    Lex {
        message: String,
        column: usize,
    },
    Parse {
        message: String,
        near: Option<String>,
    },
    Eval {
        message: String,
    },
}

impl ShellError {
    pub fn lex(message: impl Into<String>, column: usize) -> Self {
        ShellError::Lex {
            message: message.into(),
            column,
        }
    }

    pub fn parse(message: impl Into<String>, near: Option<String>) -> Self {
        ShellError::Parse {
            message: message.into(),
            near,
        }
    }

    pub fn eval(message: impl Into<String>) -> Self {
        ShellError::Eval {
            message: message.into(),
        }
    }
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellError::Lex { message, column } => {
                write!(f, "shell: lex error at column {}: {}", column, message)
            }
            ShellError::Parse { message, near } => match near {
                Some(near) => write!(f, "shell: parse error near '{}': {}", near, message),
                None => write!(f, "shell: parse error: {}", message),
            },
            ShellError::Eval { message } => write!(f, "shell: {}", message),
        }
    }
}

impl std::error::Error for ShellError {}
