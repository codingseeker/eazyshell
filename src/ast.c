#define _GNU_SOURCE
#include <stdlib.h>
#include <unistd.h>
#include "ast.h"

void free_command(Command *cmd) {
    if (!cmd) return;

    if (cmd->argv) {
        for (int i = 0; cmd->argv[i]; i++) free(cmd->argv[i]);
        free(cmd->argv);
    }

    if (cmd->infile) free(cmd->infile);
    if (cmd->outfile) free(cmd->outfile);
    if (cmd->heredoc_delim) free(cmd->heredoc_delim);

    if (cmd->heredoc_fd >= 0) close(cmd->heredoc_fd);

    free(cmd);
}

void free_pipeline(Pipeline *p) {
    if (!p) return;

    if (p->cmds) {
        for (int i = 0; i < p->count; i++) {
            if (p->cmds[i]) free_command(p->cmds[i]);
        }
        free(p->cmds);
    }

    free(p);
}

void free_ast(ASTNode *node) {
    if (!node) return;

    switch (node->type) {
        case NODE_COMMAND:
            free_command(node->data.cmd);
            break;

        case NODE_PIPELINE:
            free_pipeline(node->data.pipeline);
            break;

        case NODE_AND:
        case NODE_OR:
        case NODE_SEMICOLON:
            free_ast(node->data.binary.left);
            free_ast(node->data.binary.right);
            break;

        case NODE_IF:
            free_ast(node->data.if_stmt.condition);
            free_ast(node->data.if_stmt.then_body);
            free_ast(node->data.if_stmt.else_body);
            break;

        case NODE_WHILE:
            free_ast(node->data.while_stmt.condition);
            free_ast(node->data.while_stmt.body);
            break;

        case NODE_FOR:
            if (node->data.for_stmt.var) free(node->data.for_stmt.var);
            if (node->data.for_stmt.wordlist) {
                for (int i = 0; i < node->data.for_stmt.wordcount; i++)
                    free(node->data.for_stmt.wordlist[i]);
                free(node->data.for_stmt.wordlist);
            }
            free_ast(node->data.for_stmt.body);
            break;

        case NODE_GROUP:
            free_ast(node->data.group.body);
            break;

        default:
            break;
    }

    free(node);
}