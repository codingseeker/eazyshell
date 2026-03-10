parser.c#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include "parser.h"
#include "AST.h"

#define INIT_ARGV_CAP 8
#define MAX_PIPE_CMDS 1024
#define MAX_PARSE_DEPTH 256

static const char *token_str_at(Token *tokens, int pos, int end) {
    return (pos < end && tokens[pos].value) ? tokens[pos].value : "EOF";
}

static int is_command_terminator(TokenType t) {
    return t == TOKEN_PIPE ||
           t == TOKEN_AND ||
           t == TOKEN_OR ||
           t == TOKEN_SEMICOLON ||
           t == TOKEN_GROUP_CLOSE ||
           t == TOKEN_SUBSHELL_CLOSE ||
           t == TOKEN_THEN ||
           t == TOKEN_DO ||
           t == TOKEN_ELSE ||
           t == TOKEN_FI ||
           t == TOKEN_DONE;
}

static char *expect_word(Token *tokens, int *pos, int end) {
    if (*pos >= end || tokens[*pos].type != TOKEN_WORD) {
        fprintf(stderr, "parse error near token '%s': expected word\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    char *word = strdup(tokens[*pos].value);
    if (!word) { perror("strdup"); exit(1); }
    (*pos)++;
    return word;
}

static int command_is_empty(Command *cmd) {
    return cmd->argc == 0 && cmd->env_count == 0 &&
           !cmd->infile && !cmd->outfile && !cmd->heredoc_delim;
}

static ASTNode *parse_component(Token *tokens, int *pos, int end, int depth);
static Command *parse_command(Token *tokens, int *pos, int end);
static ASTNode *parse_pipeline(Token *tokens, int *pos, int end, int depth);
static ASTNode *parse_if(Token *tokens, int *pos, int end, int depth);
static ASTNode *parse_while(Token *tokens, int *pos, int end, int depth);
static ASTNode *parse_for(Token *tokens, int *pos, int end, int depth);
static ASTNode *parse_group(Token *tokens, int *pos, int end, int depth);
static ASTNode *parse_subshell(Token *tokens, int *pos, int end, int depth);
static ASTNode *parse_secondary(Token *tokens, int *pos, int end, int depth);

static Command *parse_command(Token *tokens, int *pos, int end) {
    Command *cmd = new_command();
    if (!cmd) { perror("new_command"); exit(1); }

    while (*pos < end) {
        TokenType type = tokens[*pos].type;
        if (is_command_terminator(type)) break;

        Token t = tokens[*pos];

        switch (type) {
            case TOKEN_WORD:
            case TOKEN_ARITH:
                if (cmd->argc + 1 >= cmd->argv_cap) {
                    size_t new_cap = cmd->argv_cap ? cmd->argv_cap * 2 : INIT_ARGV_CAP;
                    char **new_argv = realloc(cmd->argv, sizeof(char*) * new_cap);
                    if (!new_argv) { perror("realloc"); exit(1); }
                    cmd->argv = new_argv;
                    cmd->argv_cap = new_cap;
                }
                cmd->argv[cmd->argc] = strdup(t.value);
                if (!cmd->argv[cmd->argc]) { perror("strdup"); exit(1); }
                cmd->argc++;
                cmd->argv[cmd->argc] = NULL;
                (*pos)++;
                break;

            case TOKEN_ASSIGNMENT: {
                char **new_env = realloc(cmd->env_assigns,
                                         sizeof(char*) * (cmd->env_count + 1));
                if (!new_env) { perror("realloc"); exit(1); }
                cmd->env_assigns = new_env;
                cmd->env_assigns[cmd->env_count] = strdup(t.value);
                if (!cmd->env_assigns[cmd->env_count]) { perror("strdup"); exit(1); }
                cmd->env_count++;
                (*pos)++;
                break;
            }

            case TOKEN_REDIR_IN:
                (*pos)++;
                if (cmd->infile) free(cmd->infile);
                cmd->infile = expect_word(tokens, pos, end);
                break;

            case TOKEN_REDIR_OUT:
                (*pos)++;
                if (cmd->outfile) free(cmd->outfile);
                cmd->outfile = expect_word(tokens, pos, end);
                cmd->append_out = 0;
                break;

            case TOKEN_REDIR_APPEND:
                (*pos)++;
                if (cmd->outfile) free(cmd->outfile);
                cmd->outfile = expect_word(tokens, pos, end);
                cmd->append_out = 1;
                break;

            case TOKEN_REDIR_HEREDOC:
                (*pos)++;
                if (cmd->heredoc_delim) free(cmd->heredoc_delim);
                cmd->heredoc_delim = expect_word(tokens, pos, end);
                break;

            default:
                goto done;
        }
    }

done:
    return cmd;
}

static ASTNode *parse_secondary(Token *tokens, int *pos, int end, int depth) {
    TokenType next = tokens[*pos].type;
    if (next == TOKEN_IF) {
        return parse_if(tokens, pos, end, depth);
    } else if (next == TOKEN_WHILE) {
        return parse_while(tokens, pos, end, depth);
    } else if (next == TOKEN_FOR) {
        return parse_for(tokens, pos, end, depth);
    } else {
        return parse_pipeline(tokens, pos, end, depth);
    }
}

static ASTNode *parse_component(Token *tokens, int *pos, int end, int depth) {
    if (*pos >= end) return NULL;

    TokenType type = tokens[*pos].type;
    switch (type) {
        case TOKEN_IF:          return parse_if(tokens, pos, end, depth);
        case TOKEN_WHILE:       return parse_while(tokens, pos, end, depth);
        case TOKEN_FOR:         return parse_for(tokens, pos, end, depth);
        case TOKEN_SUBSHELL_OPEN: return parse_subshell(tokens, pos, end, depth);
        case TOKEN_GROUP_OPEN:  return parse_group(tokens, pos, end, depth);
        default: {
            Command *cmd = parse_command(tokens, pos, end);
            if (command_is_empty(cmd)) {
                free_command(cmd);
                fprintf(stderr, "parse error near token '%s': empty command\n",
                        token_str_at(tokens, *pos, end));
                exit(1);
            }
            ASTNode *node = new_command_node(cmd);
            if (!node) { perror("new_command_node"); exit(1); }
            return node;
        }
    }
}

static ASTNode *parse_pipeline(Token *tokens, int *pos, int end, int depth) {
    if (depth > MAX_PARSE_DEPTH) {
        fprintf(stderr, "parse error near token '%s': input nested too deeply\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }

    ASTNode *first = parse_component(tokens, pos, end, depth + 1);
    if (!first) return NULL;

    if (*pos >= end || tokens[*pos].type != TOKEN_PIPE) {
        return first;
    }

    Pipeline *p = new_pipeline();
    if (!p) { perror("new_pipeline"); exit(1); }

    size_t cap = 4;
    p->nodes = malloc(sizeof(ASTNode*) * cap);
    if (!p->nodes) { perror("malloc"); exit(1); }
    p->count = 0;
    p->nodes[p->count++] = first;

    while (*pos < end && tokens[*pos].type == TOKEN_PIPE) {
        if (p->count >= MAX_PIPE_CMDS) {
            fprintf(stderr, "parse error near token '%s': pipeline too long\n",
                    token_str_at(tokens, *pos, end));
            exit(1);
        }
        (*pos)++;
        ASTNode *next = parse_component(tokens, pos, end, depth + 1);
        if (!next) {
            fprintf(stderr, "parse error near token '%s': expected command after pipe\n",
                    token_str_at(tokens, *pos, end));
            exit(1);
        }
        if (p->count >= cap) {
            cap *= 2;
            ASTNode **new_nodes = realloc(p->nodes, sizeof(ASTNode*) * cap);
            if (!new_nodes) { perror("realloc"); exit(1); }
            p->nodes = new_nodes;
        }
        p->nodes[p->count++] = next;
    }

    ASTNode *node = new_pipeline_node(p);
    if (!node) { perror("new_pipeline_node"); exit(1); }
    return node;
}

static ASTNode *parse_subshell(Token *tokens, int *pos, int end, int depth) {
    if (depth > MAX_PARSE_DEPTH) {
        fprintf(stderr, "parse error near token '%s': input nested too deeply\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    (*pos)++;
    int sub_start = *pos;
    int depth_count = 1;
    int sub_end = sub_start;

    while (sub_end < end) {
        if (tokens[sub_end].type == TOKEN_SUBSHELL_OPEN) depth_count++;
        else if (tokens[sub_end].type == TOKEN_SUBSHELL_CLOSE) {
            depth_count--;
            if (depth_count == 0) break;
        }
        sub_end++;
    }
    if (sub_end >= end || tokens[sub_end].type != TOKEN_SUBSHELL_CLOSE) {
        fprintf(stderr, "parse error near token '%s': missing ')'\n",
                token_str_at(tokens, sub_end, end));
        exit(1);
    }

    int inner_pos = sub_start;
    ASTNode *inner = parse_toplevel(tokens, &inner_pos, sub_end, depth + 1);
    if (inner_pos != sub_end) {
        fprintf(stderr, "parse error near token '%s': garbage at end of subshell\n",
                token_str_at(tokens, inner_pos, end));
        exit(1);
    }
    *pos = sub_end + 1;

    ASTNode *node = new_subshell_node(inner);
    if (!node) { perror("new_subshell_node"); exit(1); }
    return node;
}

static ASTNode *parse_group(Token *tokens, int *pos, int end, int depth) {
    if (depth > MAX_PARSE_DEPTH) {
        fprintf(stderr, "parse error near token '%s': input nested too deeply\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    (*pos)++;
    ASTNode *body = parse_toplevel(tokens, pos, end, depth + 1);
    if (*pos >= end || tokens[*pos].type != TOKEN_GROUP_CLOSE) {
        fprintf(stderr, "parse error near token '%s': expected ')'\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    (*pos)++;
    ASTNode *node = new_group_node(body);
    if (!node) { perror("new_group_node"); exit(1); }
    return node;
}

static ASTNode *parse_if(Token *tokens, int *pos, int end, int depth) {
    if (depth > MAX_PARSE_DEPTH) {
        fprintf(stderr, "parse error near token '%s': input nested too deeply\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    (*pos)++;
    ASTNode *cond = parse_toplevel(tokens, pos, end, depth + 1);
    if (*pos >= end || tokens[*pos].type != TOKEN_THEN) {
        fprintf(stderr, "parse error near token '%s': expected 'then'\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    (*pos)++;
    ASTNode *then_body = parse_toplevel(tokens, pos, end, depth + 1);
    ASTNode *else_body = NULL;
    if (*pos < end && tokens[*pos].type == TOKEN_ELSE) {
        (*pos)++;
        else_body = parse_toplevel(tokens, pos, end, depth + 1);
    }
    if (*pos >= end || tokens[*pos].type != TOKEN_FI) {
        fprintf(stderr, "parse error near token '%s': expected 'fi'\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    (*pos)++;
    ASTNode *node = new_if_node(cond, then_body, else_body);
    if (!node) { perror("new_if_node"); exit(1); }
    return node;
}

static ASTNode *parse_while(Token *tokens, int *pos, int end, int depth) {
    if (depth > MAX_PARSE_DEPTH) {
        fprintf(stderr, "parse error near token '%s': input nested too deeply\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    (*pos)++;
    ASTNode *cond = parse_toplevel(tokens, pos, end, depth + 1);
    if (*pos >= end || tokens[*pos].type != TOKEN_DO) {
        fprintf(stderr, "parse error near token '%s': expected 'do'\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    (*pos)++;
    ASTNode *body = parse_toplevel(tokens, pos, end, depth + 1);
    if (*pos >= end || tokens[*pos].type != TOKEN_DONE) {
        fprintf(stderr, "parse error near token '%s': expected 'done'\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    (*pos)++;
    ASTNode *node = new_while_node(cond, body);
    if (!node) { perror("new_while_node"); exit(1); }
    return node;
}

static ASTNode *parse_for(Token *tokens, int *pos, int end, int depth) {
    if (depth > MAX_PARSE_DEPTH) {
        fprintf(stderr, "parse error near token '%s': input nested too deeply\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    (*pos)++;
    if (*pos >= end || tokens[*pos].type != TOKEN_WORD) {
        fprintf(stderr, "parse error near token '%s': expected variable name\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    char *var = strdup(tokens[*pos].value);
    if (!var) { perror("strdup"); exit(1); }
    (*pos)++;

    char **wordlist = NULL;
    size_t wordcount = 0;
    if (*pos < end && tokens[*pos].type == TOKEN_IN) {
        (*pos)++;
        while (*pos < end && tokens[*pos].type == TOKEN_WORD) {
            char **new_wl = realloc(wordlist, sizeof(char*) * (wordcount + 1));
            if (!new_wl) { perror("realloc"); exit(1); }
            wordlist = new_wl;
            wordlist[wordcount] = strdup(tokens[*pos].value);
            if (!wordlist[wordcount]) { perror("strdup"); exit(1); }
            wordcount++;
            (*pos)++;
        }
    }

    if (*pos >= end || tokens[*pos].type != TOKEN_DO) {
        fprintf(stderr, "parse error near token '%s': expected 'do'\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    (*pos)++;
    ASTNode *body = parse_toplevel(tokens, pos, end, depth + 1);
    if (*pos >= end || tokens[*pos].type != TOKEN_DONE) {
        fprintf(stderr, "parse error near token '%s': expected 'done'\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }
    (*pos)++;

    ASTNode *node = new_for_node(var, wordcount, wordlist, body);
    if (!node) { perror("new_for_node"); exit(1); }
    return node;
}

ASTNode *parse_toplevel(Token *tokens, int *pos, int end, int depth) {
    if (depth > MAX_PARSE_DEPTH) {
        fprintf(stderr, "parse error near token '%s': input nested too deeply\n",
                token_str_at(tokens, *pos, end));
        exit(1);
    }

    ASTNode *left = NULL;

    if (*pos >= end) return NULL;

    TokenType start = tokens[*pos].type;
    if (start == TOKEN_IF) {
        left = parse_if(tokens, pos, end, depth + 1);
    } else if (start == TOKEN_WHILE) {
        left = parse_while(tokens, pos, end, depth + 1);
    } else if (start == TOKEN_FOR) {
        left = parse_for(tokens, pos, end, depth + 1);
    } else {
        left = parse_pipeline(tokens, pos, end, depth + 1);
    }

    while (*pos < end) {
        TokenType op = tokens[*pos].type;
        if (op == TOKEN_AND || op == TOKEN_OR || op == TOKEN_SEMICOLON) {
            (*pos)++;
            ASTNode *right = NULL;
            if (*pos < end) {
                right = parse_secondary(tokens, pos, end, depth + 1);
            }
            NodeType node_type = (op == TOKEN_AND) ? NODE_AND :
                                 (op == TOKEN_OR)  ? NODE_OR  :
                                                      NODE_SEMICOLON;
            ASTNode *node = new_binary_node(node_type, left, right);
            if (!node) { perror("new_binary_node"); exit(1); }
            left = node;
        } else {
            break;
        }
    }

    return left;
}