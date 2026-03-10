 #ifndef EAZYSHELL_LEXER_H
#define EAZYSHELL_LEXER_H

#include <stddef.h>
#include <stdbool.h>

typedef enum {
TOKEN_WORD,
TOKEN_VAR,
TOKEN_VAR_BRACE,
TOKEN_ARITH,
TOKEN_GROUP_OPEN,
TOKEN_GROUP_CLOSE,
TOKEN_SUBSHELL_OPEN,
TOKEN_SUBSHELL_CLOSE,
TOKEN_PIPE,
TOKEN_AND,
TOKEN_OR,
TOKEN_SEMICOLON,
TOKEN_BACKGROUND,
TOKEN_REDIR_IN,
TOKEN_REDIR_OUT,
TOKEN_REDIR_APPEND,
TOKEN_REDIR_CLOBBER,
TOKEN_REDIR_BOTH_OUT,
TOKEN_REDIR_HEREDOC,
TOKEN_REDIR_HEREDOC_TAB,
TOKEN_EOF  /* synthetic token appended at end of stream */
} TokenType;

typedef struct {
TokenType type;      /* kind of token */
char value;         / heap string owned by Token; freed by free_tokens() /
bool quoted;         / true if token originated inside quotes */
} Token;

/* tokenize() returns a heap-allocated token array that includes a final TOKEN_EOF.
*ntok will be set to the total number of tokens, including the EOF token.
Caller must free the array with free_tokens(). */
Token *tokenize(char *line, size_t *ntok);
void free_tokens(Token *tokens, size_t count);

#endif