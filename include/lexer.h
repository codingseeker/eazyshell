#ifndef LEXER_H
#define LEXER_H

#include "utils.h"

typedef enum {
    TOKEN_WORD,
    TOKEN_PIPE,
    TOKEN_AND,
    TOKEN_OR,
    TOKEN_SEMICOLON,
    TOKEN_REDIR_IN,
    TOKEN_REDIR_OUT,
    TOKEN_REDIR_HEREDOC,
    TOKEN_BACKGROUND,
    TOKEN_SUBSHELL_OPEN,
    TOKEN_SUBSHELL_CLOSE,
    TOKEN_ASSIGNMENT,
    TOKEN_ARITH,
    TOKEN_IF,
    TOKEN_THEN,
    TOKEN_ELSE,
    TOKEN_ELIF,
    TOKEN_FI,
    TOKEN_WHILE,
    TOKEN_DO,
    TOKEN_DONE,
    TOKEN_FOR,
    TOKEN_IN,
    TOKEN_GROUP_OPEN,
    TOKEN_GROUP_CLOSE,
    TOKEN_EOF
} TokenType;

typedef struct Token {
    TokenType type;
    char *value;
    int quoted;
} Token;

Token *tokenize(char *line, int *ntok);

#endif