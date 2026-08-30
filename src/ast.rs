#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub argv: Vec<String>,
    pub env_assigns: Vec<String>,
    pub infile: Option<String>,
    pub outfile: Option<String>,
    pub append_out: bool,
    pub heredoc_delim: Option<String>,
}

impl Command {
    pub fn new() -> Self {
        Command {
            argv: Vec::new(),
            env_assigns: Vec::new(),
            infile: None,
            outfile: None,
            append_out: false,
            heredoc_delim: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.argv.is_empty()
            && self.env_assigns.is_empty()
            && self.infile.is_none()
            && self.outfile.is_none()
            && self.heredoc_delim.is_none()
    }
}

impl Default for Command {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pipeline {
    pub commands: Vec<Command>,
}

impl Pipeline {
    pub fn new() -> Self {
        Pipeline {
            commands: Vec::new(),
        }
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfStmt {
    pub condition: Box<AstNode>,
    pub then_body: Box<AstNode>,
    pub else_body: Option<Box<AstNode>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhileStmt {
    pub condition: Box<AstNode>,
    pub body: Box<AstNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForStmt {
    pub var: String,
    pub wordlist: Vec<String>,
    pub body: Box<AstNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AstNode {
    Command {
        command: Command,
    },
    Pipeline {
        pipeline: Pipeline,
    },
    And {
        left: Box<AstNode>,
        right: Box<AstNode>,
    },
    Or {
        left: Box<AstNode>,
        right: Box<AstNode>,
    },
    Semicolon {
        left: Box<AstNode>,
        right: Box<AstNode>,
    },
    Background {
        body: Box<AstNode>,
    },
    #[allow(dead_code)]
    If(IfStmt),
    #[allow(dead_code)]
    While(WhileStmt),
    #[allow(dead_code)]
    For(ForStmt),
    Group {
        body: Box<AstNode>,
    },
}
impl AstNode {
    pub fn command(command: Command) -> Self {
        AstNode::Command { command }
    }

    pub fn pipeline(pipeline: Pipeline) -> Self {
        AstNode::Pipeline { pipeline }
    }

    pub fn binary_and(left: AstNode, right: AstNode) -> Self {
        AstNode::And {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    pub fn binary_or(left: AstNode, right: AstNode) -> Self {
        AstNode::Or {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    pub fn binary_semicolon(left: AstNode, right: AstNode) -> Self {
        AstNode::Semicolon {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    pub fn background(body: AstNode) -> Self {
        AstNode::Background {
            body: Box::new(body),
        }
    }

    pub fn group(body: AstNode) -> Self {
        AstNode::Group {
            body: Box::new(body),
        }
    }
}
