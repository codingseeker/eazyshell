 #define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include "lexer.h"

#define INIT_TOKEN_CAP 32
#define MAX_SUBSHELL_DEPTH 64

static int is_operator_char(char c) {
    return strchr("|&;<>()", c) != NULL;
}

static void ensure_capacity(Token **tokens, int *cap, int idx, int need) {
    if (idx + need > *cap) {
        while (idx + need > *cap) *cap *= 2;
        *tokens = realloc(*tokens, sizeof(Token) * (*cap));
        if (!*tokens) { perror("realloc"); exit(1); }
    }
}

static void lex_error(const char *line, int col, const char *msg) {
    fprintf(stderr, "lex error at column %d: %s\n", col, msg);
    exit(1);
}

static char *read_variable_name(const char *start, const char **endp) {
    const char *p = start;
    if (isdigit(*p)) {
        p++;
    } else if (isalpha(*p) || *p == '_') {
        p++;
        while (isalnum(*p) || *p == '_') p++;
    } else if (*p == '?' || *p == '$' || *p == '!' || *p == '*' || *p == '@') {
        p++;
    } else {
        return NULL;
    }
    *endp = p;
    return strndup(start, p - start);
}

static char *read_braced_variable(const char *start, const char **endp, int *error) {
    const char *p = start;
    if (*p != '{') return NULL;
    p++;
    const char *var_start = p;
    int depth = 1;
    while (*p) {
        if (*p == '{') depth++;
        else if (*p == '}') {
            depth--;
            if (depth == 0) break;
        }
        p++;
    }
    if (depth != 0) {
        *error = 1;
        return NULL;
    }
    *endp = p + 1;
    return strndup(var_start, p - var_start);
}

static char *read_word(const char *line, int *pos, int line_len, int *col, int *quoted) {
    int in_squote = 0, in_dquote = 0;
    int escaped = 0;
    *quoted = 0;
    size_t cap = 16, len = 0;
    char *word = malloc(cap);
    if (!word) { perror("malloc"); exit(1); }
    while (*pos < line_len) {
        char c = line[*pos];
        if (escaped) {
            if (len + 1 >= cap) { cap *= 2; word = realloc(word, cap); if (!word) { perror("realloc"); exit(1); } }
            word[len++] = c;
            escaped = 0;
            (*pos)++;
            (*col)++;
            continue;
        }
        if (c == '\\' && !in_squote) {
            escaped = 1;
            (*pos)++;
            (*col)++;
            continue;
        }
        if (c == '\'' && !in_dquote) {
            in_squote = !in_squote;
            *quoted = 1;
            (*pos)++;
            (*col)++;
            continue;
        }
        if (c == '"' && !in_squote) {
            in_dquote = !in_dquote;
            *quoted = 1;
            (*pos)++;
            (*col)++;
            continue;
        }
        if (!in_squote && !in_dquote) {
            if (is_operator_char(c) || c == ' ' || c == '\t')
                break;
            if (c == '$' && (*pos + 1 < line_len)) {
                char n = line[*pos + 1];
                if (n == '(' || n == '{' || isalpha(n) || n == '_' || n == '?' || n == '$' || n == '!' || n == '*' || n == '@' || isdigit(n))
                    break;
            }
        }
        if (len + 1 >= cap) { cap *= 2; word = realloc(word, cap); if (!word) { perror("realloc"); exit(1); } }
        word[len++] = c;
        (*pos)++;
        (*col)++;
    }
    if (in_squote || in_dquote) {
        free(word);
        return NULL;
    }
    if (escaped) {
        free(word);
        return NULL;
    }
    word[len] = '\0';
    return word;
}

static Token *tokenize_internal(char *line, int depth, int *ntok);

Token *tokenize(char *line, int *ntok) {
    return tokenize_internal(line, 0, ntok);
}

static Token *tokenize_internal(char *line, int depth, int *ntok) {
    int cap = INIT_TOKEN_CAP;
    Token *tokens = malloc(sizeof(Token) * cap);
    if (!tokens) { perror("malloc"); exit(1); }
    int i = 0;
    int pos = 0;
    int col = 1;
    int line_len = strlen(line);
    while (pos < line_len) {
        char c = line[pos];
        if (c == ' ' || c == '\t') {
            pos++;
            col++;
            continue;
        }
        if (c == '#' && (pos == 0 || line[pos-1] == ' ' || line[pos-1] == '\t')) {
            break;
        }
        char c1 = (pos + 1 < line_len) ? line[pos + 1] : 0;
        if (c == '|' && c1 == '|') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_OR, .value = NULL, .quoted = 0 };
            pos += 2; col += 2;
            continue;
        }
        if (c == '|') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_PIPE, .value = NULL, .quoted = 0 };
            pos++; col++;
            continue;
        }
        if (c == '&' && c1 == '&') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_AND, .value = NULL, .quoted = 0 };
            pos += 2; col += 2;
            continue;
        }
        if (c == ';') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_SEMICOLON, .value = NULL, .quoted = 0 };
            pos++; col++;
            continue;
        }
        if (c == '&' && c1 == '>') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_REDIR_BOTH_OUT, .value = NULL, .quoted = 0 };
            pos += 2; col += 2;
            continue;
        }
        if (c == '>' && c1 == '>') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_REDIR_APPEND, .value = NULL, .quoted = 0 };
            pos += 2; col += 2;
            continue;
        }
        if (c == '>' && c1 == '|') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_REDIR_CLOBBER, .value = NULL, .quoted = 0 };
            pos += 2; col += 2;
            continue;
        }
        if (c == '<' && c1 == '<' && pos + 2 < line_len && line[pos+2] == '-') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_REDIR_HEREDOC_TAB, .value = NULL, .quoted = 0 };
            pos += 3; col += 3;
            continue;
        }
        if (c == '<' && c1 == '<') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_REDIR_HEREDOC, .value = NULL, .quoted = 0 };
            pos += 2; col += 2;
            continue;
        }
        if (c == '<') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_REDIR_IN, .value = NULL, .quoted = 0 };
            pos++; col++;
            continue;
        }
        if (c == '>') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_REDIR_OUT, .value = NULL, .quoted = 0 };
            pos++; col++;
            continue;
        }
        if (c == '&' && c1 != '&') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_BACKGROUND, .value = NULL, .quoted = 0 };
            pos++; col++;
            continue;
        }
        if (c == '$' && c1 == '(' && pos + 2 < line_len && line[pos+2] == '(') {
            pos += 3; col += 3;
            const char *start = line + pos;
            int paren_depth = 2;
            while (pos < line_len) {
                char c = line[pos];
                if (c == '(') paren_depth++;
                else if (c == ')') paren_depth--;
                if (paren_depth == 0) break;
                pos++; col++;
            }
            if (paren_depth != 0) {
                for (int j = 0; j < i; j++) free(tokens[j].value);
                free(tokens);
                lex_error(line, col, "unfinished $(( expression");
            }
            char *expr = strndup(start, line + pos - start);
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_ARITH, .value = expr, .quoted = 0 };
            pos++; col++;
            continue;
        }
        if (c == '$' && c1 == '(') {
            if (++depth > MAX_SUBSHELL_DEPTH) {
                for (int j = 0; j < i; j++) free(tokens[j].value);
                free(tokens);
                lex_error(line, col, "subshell nesting too deep");
            }
            pos += 2; col += 2;
            const char *start = line + pos;
            int paren_depth = 1;
            while (pos < line_len) {
                char c = line[pos];
                if (c == '(') paren_depth++;
                else if (c == ')') paren_depth--;
                if (paren_depth == 0) break;
                pos++; col++;
            }
            if (paren_depth != 0) {
                depth--;
                for (int j = 0; j < i; j++) free(tokens[j].value);
                free(tokens);
                lex_error(line, col, "unfinished $(");
            }
            int inner_len = line + pos - start;
            char *inner = strndup(start, inner_len);
            int inner_ntok;
            Token *inner_tokens = tokenize_internal(inner, depth, &inner_ntok);
            free(inner);
            ensure_capacity(&tokens, &cap, i, 1 + inner_ntok + 1);
            tokens[i++] = (Token){ .type = TOKEN_SUBSHELL_OPEN, .value = NULL, .quoted = 0 };
            for (int j = 0; j < inner_ntok; j++) {
                tokens[i++] = inner_tokens[j];
            }
            free(inner_tokens);
            tokens[i++] = (Token){ .type = TOKEN_SUBSHELL_CLOSE, .value = NULL, .quoted = 0 };
            pos++; col++;
            depth--;
            continue;
        }
        if (c == '$' && (c1 == '{' || isalpha(c1) || c1 == '_' || c1 == '?' || c1 == '$' || c1 == '!' || c1 == '*' || c1 == '@' || isdigit(c1))) {
            if (c1 == '{') {
                const char *start = line + pos + 2;
                int error = 0;
                const char *end;
                char *var = read_braced_variable(start, &end, &error);
                if (error) {
                    for (int j = 0; j < i; j++) free(tokens[j].value);
                    free(tokens);
                    lex_error(line, col, "unclosed ${");
                }
                ensure_capacity(&tokens, &cap, i, 1);
                tokens[i++] = (Token){ .type = TOKEN_VAR_BRACE, .value = var, .quoted = 0 };
                int consumed = end - line;
                col += (consumed - pos);
                pos = consumed;
            } else {
                const char *start = line + pos + 1;
                const char *end;
                char *var = read_variable_name(start, &end);
                if (!var) {
                    for (int j = 0; j < i; j++) free(tokens[j].value);
                    free(tokens);
                    lex_error(line, col, "invalid variable name");
                }
                ensure_capacity(&tokens, &cap, i, 1);
                tokens[i++] = (Token){ .type = TOKEN_VAR, .value = var, .quoted = 0 };
                int consumed = end - line;
                col += (consumed - pos);
                pos = consumed;
            }
            continue;
        }
        if (c == '(') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_GROUP_OPEN, .value = NULL, .quoted = 0 };
            pos++; col++;
            continue;
        }
        if (c == ')') {
            ensure_capacity(&tokens, &cap, i, 1);
            tokens[i++] = (Token){ .type = TOKEN_GROUP_CLOSE, .value = NULL, .quoted = 0 };
            pos++; col++;
            continue;
        }
        int quoted;
        char *word = read_word(line, &pos, line_len, &col, &quoted);
        if (!word) {
            for (int j = 0; j < i; j++) free(tokens[j].value);
            free(tokens);
            lex_error(line, col, "unclosed quote or trailing escape");
        }
        ensure_capacity(&tokens, &cap, i, 1);
        tokens[i++] = (Token){ .type = TOKEN_WORD, .value = word, .quoted = quoted };
    }
    *ntok = i;
    return tokens;
}