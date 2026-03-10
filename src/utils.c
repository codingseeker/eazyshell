#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <readline/readline.h>
#include <readline/history.h>
#include "lexer.h"
#include "parser.h"
#include "executor.h"
#include "job_control.h"
#include "builtins.h"

static char *expand_aliases(const char *word) {
    for (int i = 0; i < alias_count; i++) {
        if (strcmp(aliases[i].name, word) == 0)
            return strdup(aliases[i].value);
    }
    return NULL;
}

int main(void) {
    setup_signals();
    rl_readline_name = "eazyshell";
    using_history();
    char *line;
    while (1) {
        line = readline("eazyshell> ");
        if (!line) break;
        if (strlen(line) > 0) add_history(line);
        int ntok;
        Token *tokens = tokenize(line, &ntok);
        if (ntok > 0 && tokens[0].type == TOKEN_WORD && !tokens[0].quoted) {
            char *alias_val = expand_aliases(tokens[0].value);
            if (alias_val) {
                free(tokens[0].value);
                tokens[0].value = alias_val;
            }
        }
        int pos = 0;
        ASTNode *ast = parse_toplevel(tokens, &pos, ntok);
        int bg = 0;
        if (pos < ntok && tokens[pos].type == TOKEN_BACKGROUND) bg = 1;
        if (ast) {
            eval_ast(ast, bg, 0);
            free_ast(ast);
        }
        free_tokens(tokens, ntok);
        free(line);
    }
    return 0;
}