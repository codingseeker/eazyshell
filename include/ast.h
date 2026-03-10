#ifndef AST_H
#define AST_H

#include "lexer.h"

typedef struct Command {
    char **argv;
    int argc;
    int argv_cap;
    char *infile;
    char *outfile;
    char *heredoc_delim;
    int heredoc_fd;
} Command;

typedef struct Pipeline {
    Command **cmds;
    int count;
} Pipeline;

typedef enum {
    NODE_COMMAND,
    NODE_PIPELINE,
    NODE_AND,
    NODE_OR,
    NODE_SEMICOLON,
    NODE_IF,
    NODE_WHILE,
    NODE_FOR,
    NODE_GROUP
} NodeType;

typedef struct ASTNode {
    NodeType type;
    union {
        Command *cmd;
        struct {
            struct ASTNode *left;
            struct ASTNode *right;
        } binary;
        Pipeline *pipeline;
        struct {
            struct ASTNode *condition;
            struct ASTNode *then_body;
            struct ASTNode *else_body;
        } if_stmt;
        struct {
            struct ASTNode *condition;
            struct ASTNode *body;
        } while_stmt;
        struct {
            char *var;
            char **wordlist;
            int wordcount;
            struct ASTNode *body;
        } for_stmt;
        struct {
            struct ASTNode *body;
        } group;
    } data;
} ASTNode;

void free_command(Command *cmd);
void free_pipeline(Pipeline *p);
void free_ast(ASTNode *node);

#endif