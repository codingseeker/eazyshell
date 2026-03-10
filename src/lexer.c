#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include "lexer.h"
#include "expansion.h"

#define MAX_TOKENS 256
#define MAX_VAR_NAME 256

Token *tokenize(char *line, int *ntok) {
    Token *tokens = malloc(sizeof(Token) * MAX_TOKENS);
    if (!tokens) { perror("malloc"); exit(1); }
    int i = 0;
    char *p = line;
    while (*p) {
        while (*p == ' ' || *p == '\t') p++;
        if (!*p) break;
        if (*p == '|' && *(p+1) == '|') {
            tokens[i].type = TOKEN_OR;
            tokens[i].value = NULL;
            tokens[i].quoted = 0;
            p += 2;
        } else if (*p == '|') {
            tokens[i].type = TOKEN_PIPE;
            tokens[i].value = NULL;
            tokens[i].quoted = 0;
            p++;
        } else if (*p == '&' && *(p+1) == '&') {
            tokens[i].type = TOKEN_AND;
            tokens[i].value = NULL;
            tokens[i].quoted = 0;
            p += 2;
        } else if (*p == ';') {
            tokens[i].type = TOKEN_SEMICOLON;
            tokens[i].value = NULL;
            tokens[i].quoted = 0;
            p++;
        } else if (*p == '<' && *(p+1) == '<') {
            tokens[i].type = TOKEN_REDIR_HEREDOC;
            tokens[i].value = NULL;
            tokens[i].quoted = 0;
            p += 2;
        } else if (*p == '<') {
            tokens[i].type = TOKEN_REDIR_IN;
            tokens[i].value = NULL;
            tokens[i].quoted = 0;
            p++;
        } else if (*p == '>') {
            tokens[i].type = TOKEN_REDIR_OUT;
            tokens[i].value = NULL;
            tokens[i].quoted = 0;
            p++;
        } else if (*p == '&' && (*(p+1) == ' ' || *(p+1) == '\0' || *(p+1) == '\t')) {
            tokens[i].type = TOKEN_BACKGROUND;
            tokens[i].value = NULL;
            tokens[i].quoted = 0;
            p++;
        } else if (*p == '$' && *(p+1) == '(' && *(p+2) == '(') {
            p += 3;
            char *start = p;
            int depth = 1;
            while (*p) {
                if (*p == '(' && *(p+1) == '(') { depth++; p += 2; }
                else if (*p == ')' && *(p+1) == ')') {
                    depth--;
                    if (depth == 0) break;
                    p += 2;
                } else p++;
            }
            char *expr = strndup(start, p - start);
            tokens[i].type = TOKEN_ARITH;
            tokens[i].value = expr;
            tokens[i].quoted = 0;
            p += 2;
        } else if (*p == '$' && *(p+1) == '(') {
            tokens[i].type = TOKEN_SUBSHELL_OPEN;
            tokens[i].value = NULL;
            tokens[i].quoted = 0;
            p += 2;
        } else if (*p == '(') {
            tokens[i].type = TOKEN_GROUP_OPEN;
            tokens[i].value = NULL;
            tokens[i].quoted = 0;
            p++;
        } else if (*p == ')') {
            tokens[i].type = TOKEN_GROUP_CLOSE;
            tokens[i].value = NULL;
            tokens[i].quoted = 0;
            p++;
        } else {
            char *start = p;
            int in_squote = 0, in_dquote = 0;
            int escaped = 0;
            int quoted = 0;
            while (*p) {
                if (escaped) { escaped = 0; p++; continue; }
                if (*p == '\\' && !in_squote) { escaped = 1; p++; continue; }
                if (*p == '\'' && !in_dquote) { in_squote = !in_squote; quoted = 1; p++; continue; }
                if (*p == '"' && !in_squote) { in_dquote = !in_dquote; quoted = 1; p++; continue; }
                if (!in_squote && !in_dquote) {
                    if (strchr(" \t|&;<>()", *p)) break;
                    if (*p == '$' && (*(p+1) == '(' || (isalpha(*(p+1)) || *(p+1) == '_' || *(p+1) == '{'))) break;
                }
                p++;
            }
            char *word = strndup(start, p - start);
            if (!in_squote && !in_dquote) {
                char *eq = strchr(word, '=');
                if (eq && eq != word && (isalpha(word[0]) || word[0] == '_')) {
                    int valid = 1;
                    for (char *c = word; c < eq; c++) {
                        if (!isalnum(*c) && *c != '_') { valid = 0; break; }
                    }
                    if (valid) {
                        tokens[i].type = TOKEN_ASSIGNMENT;
                        tokens[i].value = word;
                        tokens[i].quoted = quoted;
                        i++;
                        if (i >= MAX_TOKENS) break;
                        continue;
                    }
                }
            }
            char *expanded = expand_param(word);
            tokens[i].type = TOKEN_WORD;
            tokens[i].value = expanded;
            tokens[i].quoted = quoted;
            free(word);
        }
        i++;
        if (i >= MAX_TOKENS) break;
    }
    *ntok = i;
    return tokens;
}