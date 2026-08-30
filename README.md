# eazyshell

A lightweight Unix shell written in Rust, built from scratch to explore shell internals, parsing, process execution, command expansion, and Unix system programming.

## Current Architecture

```text
eazyshell/
├── src/
│   ├── main.rs
│   ├── shell.rs
│   ├── lexer.rs
│   ├── token.rs
│   ├── parser.rs
│   ├── ast.rs
│   ├── expansion.rs
│   ├── builtins.rs
│   ├── executor.rs
│   └── error.rs
├── tests/
│   └── shell_integration.rs
├── Cargo.toml
└── README.md
```

## Features

* Interactive shell
* Command parsing and tokenization
* Abstract Syntax Tree (AST)
* Command execution
* Pipelines
* Input/output redirection
* Environment variable expansion
* Command expansion
* Brace expansion
* Globbing
* Built-in commands
* Control-flow parsing
* Subshell support
* Integration tests

## Architecture

The shell follows a pipeline-based architecture:

```text
Input
  │
  ▼
Lexer
  │
  ▼
Tokens
  │
  ▼
Parser
  │
  ▼
AST
  │
  ▼
Expansion
  │
  ▼
Executor
  │
  ▼
Unix Processes
```

Each stage has a focused responsibility, keeping parsing, expansion, execution, and shell state separated.

## Build

Requirements:

* Rust
* Cargo
* Unix-like operating system

Clone the repository:

```bash
git clone git@github.com:codingseeker/eazyshell.git
cd eazyshell
```

Build:

```bash
cargo build
```

Run:

```bash
cargo run
```

For an optimized build:

```bash
cargo build --release
```

## Testing

Run the complete test suite:

```bash
cargo test
```

Run tests with output:

```bash
cargo test -- --nocapture
```

Check the project without building an executable:

```bash
cargo check
```

## Project Status

eazyshell is an educational systems-programming project and is under active development.

The current focus is improving correctness, shell semantics, error handling, process management, and test coverage rather than attempting to reproduce every feature of Bash.

## Goals

The project is intended to provide practical experience with:

* Rust systems programming
* Unix processes
* File descriptors
* Pipes
* Process groups
* Signals
* Shell parsing
* Recursive-descent parsers
* AST-based execution
* Command and parameter expansion
* Error handling
* Integration testing

## License

This project is for educational and experimental purposes.
