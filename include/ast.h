#ifndef AST_H
#define AST_H

#include <stddef.h>

typedef enum NodeType {
    NODE_COMMAND,
    NODE_PIPELINE,
    NODE_AND,
    NODE_OR,
    NODE_SEMICOLON,
    NODE_IF,
    NODE_WHILE,
    NODE_FOR,
    NODE_SUBSHELL,
    NODE_LIST,
    NODE_FUNCTION,
    NODE_CASE
} NodeType;

typedef struct Command Command;
typedef struct Pipeline Pipeline;
typedef struct ASTNode ASTNode;

struct Command {
    size_t argc;
    char **argv;
    char *infile;
    char *outfile;
    int append_out;
    char *heredoc_delim;
    int heredoc_fd;
};

struct Pipeline {
    size_t count;
    ASTNode **nodes;
};

struct ASTNode {
    NodeType type;
    union {
        Command *cmd;
        Pipeline *pipeline;
        struct {
            ASTNode *left;
            ASTNode *right;
        } binary;
        struct {
            ASTNode *condition;
            ASTNode *then_body;
            ASTNode *else_body;
        } if_stmt;
        struct {
            ASTNode *condition;
            ASTNode *body;
        } while_stmt;
        struct {
            char *var;
            size_t word_count;
            char **wordlist;
            ASTNode *body;
        } for_stmt;
        struct {
            ASTNode *body;
        } subshell;
    } data;
};

ASTNode *new_command_node(Command *cmd);
ASTNode *new_pipeline_node(Pipeline *p);
ASTNode *new_binary_node(NodeType type, ASTNode *left, ASTNode *right);
ASTNode *new_if_node(ASTNode *cond, ASTNode *then_body, ASTNode *else_body);
ASTNode *new_while_node(ASTNode *cond, ASTNode *body);
ASTNode *new_for_node(char *var, size_t word_count, char **wordlist, ASTNode *body);
ASTNode *new_subshell_node(ASTNode *body);

void free_command(Command *cmd);
void free_pipeline(Pipeline *p);
void free_ast(ASTNode *node);

#endif