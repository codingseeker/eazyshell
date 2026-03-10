eazyshell

eazyshell is a small shell written in C as a learning project. The goal of this project is to understand how Unix shells work internally by implementing basic components such as lexical analysis, parsing, command execution, and job control.

This project is intended as a systems programming exercise for learning concepts related to operating systems, process management, and command interpreters.

Objectives

The main objectives of this project are:

- Learn how a command line shell works
- Practice modular programming in C
- Understand process creation and execution using system calls
- Implement command parsing and execution
- Gain experience with building and organizing a medium-sized C project

Project Structure

The project is organized into two main directories: "include" and "src".

eazyshell/
├── include/
│   ├── ast.h
│   ├── builtins.h
│   ├── executor.h
│   ├── expansion.h
│   ├── job_control.h
│   ├── lexer.h
│   ├── parser.h
│   └── utils.h
├── src/
│   ├── ast.c
│   ├── builtins.c
│   ├── executor.c
│   ├── expansion.c
│   ├── job_control.c
│   ├── lexer.c
│   ├── main.c
│   ├── parser.c
│   └── utils.c
├── Makefile
└── README.md

include/

This directory contains all header files used in the project. These files define data structures, function declarations, and shared interfaces between different modules.

src/

This directory contains the implementation files for the shell. Each ".c" file corresponds to a specific module of the shell.

Makefile

The Makefile is used to compile the project and manage the build process.

README.md

This file provides an overview of the project, its purpose, and its structure.

Modules

The shell is divided into multiple modules to keep the code organized and easier to understand.

- Lexer – Converts the input command line into tokens.
- Parser – Builds a structured representation of the command from tokens.
- AST (Abstract Syntax Tree) – Represents commands and operations in a tree structure.
- Executor – Executes commands using system calls such as "fork", "exec", and "wait".
- Builtins – Implements built-in shell commands such as "cd" and "exit".
- Expansion – Handles variable and command expansion.
- Job Control – Manages foreground and background processes.
- Utils – Contains helper functions used across the project.

Building the Project

To build the project, run the following command from the root directory:

make

This will compile the source files and produce the shell executable.

Running the Shell

After building the project, run the shell using:

./eazyshell

The shell will start and accept commands from the user.

Learning Goals

This project helps in understanding:

- How command line interpreters work
- Process creation and management
- Input parsing techniques
- Modular C program design
- Basics of Unix system programming

Future Improvements

Some possible improvements include:

- Better error handling
- More built-in commands
- Advanced shell features such as pipes and redirection
- Command history
- Improved job control

Author

Created as a learning project for understanding shell implementation in C.