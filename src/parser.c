#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <glob.h>
#include <unistd.h>
#include <sys/wait.h>
#include "parser.h"
#include "expansion.h"
#include "executor.h"   // for eval_ast in command substitution

#define MAX_CMDS 16
#define INIT_ARGV_CAP 8

static Command *parse_command(Token *tokens, int *pos, int end);
static ASTNode *parse_pipeline(Token *tokens, int *pos, int end);
static ASTNode *parse_if(Token *tokens, int *pos, int end);
static ASTNode *parse_while(Token *tokens, int *pos, int end);
static ASTNode *parse_for(Token *tokens, int *pos, int end);
static ASTNode *parse_group(Token *tokens, int *pos, int end);

static Command *parse_command(Token *tokens, int *pos, int end) {
    Command *cmd = malloc(sizeof(Command));
    if (!cmd) { perror("malloc"); exit(1); }
    cmd->argv = malloc(sizeof(char*) * INIT_ARGV_CAP);
    if (!cmd->argv) { perror("malloc"); exit(1); }
    cmd->argc = 0;
    cmd->argv_cap = INIT_ARGV_CAP;
    cmd->infile = NULL;
    cmd->outfile = NULL;
    cmd->heredoc_delim = NULL;
    cmd->heredoc_fd = -1;
    while (*pos < end) {
        Token t = tokens[*pos];
        if (t.type == TOKEN_WORD) {
            int bcount;
            char **bwords = expand_braces(t.value, &bcount);
            for (int bi = 0; bi < bcount; bi++) {
                if (!t.quoted) {
                    int splitcount;
                    char **split = split_words(bwords[bi], &splitcount);
                    for (int si = 0; si < splitcount; si++) {
                        if (cmd->argc >= cmd->argv_cap) {
                            cmd->argv_cap *= 2;
                            cmd->argv = realloc(cmd->argv, sizeof(char*) * cmd->argv_cap);
                            if (!cmd->argv) { perror("realloc"); exit(1); }
                        }
                        glob_t globbuf;
                        int ret = glob(split[si], GLOB_NOCHECK | GLOB_TILDE, NULL, &globbuf);
                        if (ret == 0) {
                            for (size_t g = 0; g < globbuf.gl_pathc; g++) {
                                cmd->argv[cmd->argc++] = strdup(globbuf.gl_pathv[g]);
                            }
                        }
                        globfree(&globbuf);
                        free(split[si]);
                    }
                    free(split);
                } else {
                    if (cmd->argc >= cmd->argv_cap) {
                        cmd->argv_cap *= 2;
                        cmd->argv = realloc(cmd->argv, sizeof(char*) * cmd->argv_cap);
                        if (!cmd->argv) { perror("realloc"); exit(1); }
                    }
                    glob_t globbuf;
                    int ret = glob(bwords[bi], GLOB_NOCHECK | GLOB_TILDE, NULL, &globbuf);
                    if (ret == 0) {
                        for (size_t g = 0; g < globbuf.gl_pathc; g++) {
                            cmd->argv[cmd->argc++] = strdup(globbuf.gl_pathv[g]);
                        }
                    }
                    globfree(&globbuf);
                }
                free(bwords[bi]);
            }
            free(bwords);
            (*pos)++;
        } else if (t.type == TOKEN_ARITH) {
            long res = eval_arith_expr(t.value);
            char buf[64];
            snprintf(buf, sizeof(buf), "%ld", res);
            if (cmd->argc >= cmd->argv_cap) {
                cmd->argv_cap *= 2;
                cmd->argv = realloc(cmd->argv, sizeof(char*) * cmd->argv_cap);
                if (!cmd->argv) { perror("realloc"); exit(1); }
            }
            cmd->argv[cmd->argc++] = strdup(buf);
            (*pos)++;
        } else if (t.type == TOKEN_REDIR_IN) {
            (*pos)++;
            if (*pos < end && tokens[*pos].type == TOKEN_WORD) {
                if (cmd->infile) free(cmd->infile);
                cmd->infile = strdup(tokens[*pos].value);
            }
            (*pos)++;
        } else if (t.type == TOKEN_REDIR_OUT) {
            (*pos)++;
            if (*pos < end && tokens[*pos].type == TOKEN_WORD) {
                if (cmd->outfile) free(cmd->outfile);
                cmd->outfile = strdup(tokens[*pos].value);
            }
            (*pos)++;
        } else if (t.type == TOKEN_REDIR_HEREDOC) {
            (*pos)++;
            if (*pos < end && tokens[*pos].type == TOKEN_WORD) {
                if (cmd->heredoc_delim) free(cmd->heredoc_delim);
                cmd->heredoc_delim = strdup(tokens[*pos].value);
                char *delim = cmd->heredoc_delim;
                char *line = NULL;
                size_t len = 0;
                FILE *fp = tmpfile();
                if (!fp) { perror("tmpfile"); exit(1); }
                int fd = fileno(fp);
                cmd->heredoc_fd = fd;
                while (getline(&line, &len, stdin) != -1) {
                    line[strcspn(line, "\n")] = '\0';
                    if (strcmp(line, delim) == 0) break;
                    write(fd, line, strlen(line));
                    write(fd, "\n", 1);
                }
                free(line);
                lseek(fd, 0, SEEK_SET);
            }
            (*pos)++;
        } else if (t.type == TOKEN_ASSIGNMENT) {
            char *eq = strchr(t.value, '=');
            *eq = '\0';
            setenv(t.value, eq+1, 1);
            free_token(&t);
            (*pos)++;
        } else if (t.type == TOKEN_SUBSHELL_OPEN) {
            (*pos)++;
            int sub_end = *pos;
            int depth = 1;
            while (sub_end < end) {
                if (tokens[sub_end].type == TOKEN_SUBSHELL_OPEN) depth++;
                else if (tokens[sub_end].type == TOKEN_SUBSHELL_CLOSE) {
                    depth--;
                    if (depth == 0) break;
                }
                sub_end++;
            }
            int sub_pos = *pos;
            ASTNode *sub_ast = parse_toplevel(tokens, &sub_pos, sub_end);
            int pipefd[2];
            if (pipe(pipefd) < 0) { perror("pipe"); exit(1); }
            pid_t pid = fork();
            if (pid < 0) { perror("fork"); exit(1); }
            if (pid == 0) {
                close(pipefd[0]);
                dup2(pipefd[1], STDOUT_FILENO);
                close(pipefd[1]);
                eval_ast(sub_ast, 0, 0);
                exit(0);
            } else {
                close(pipefd[1]);
                char buf[1024];
                ssize_t n;
                char *output = malloc(1);
                if (!output) { perror("malloc"); exit(1); }
                output[0] = '\0';
                while ((n = read(pipefd[0], buf, sizeof(buf)-1)) > 0) {
                    buf[n] = '\0';
                    output = realloc(output, strlen(output)+n+1);
                    if (!output) { perror("realloc"); exit(1); }
                    strcat(output, buf);
                }
                waitpid(pid, NULL, 0);
                close(pipefd[0]);
                output[strcspn(output, "\n")] = '\0';
                int splitcount;
                char **split = split_words(output, &splitcount);
                for (int si = 0; si < splitcount; si++) {
                    if (cmd->argc >= cmd->argv_cap) {
                        cmd->argv_cap *= 2;
                        cmd->argv = realloc(cmd->argv, sizeof(char*) * cmd->argv_cap);
                        if (!cmd->argv) { perror("realloc"); exit(1); }
                    }
                    cmd->argv[cmd->argc++] = split[si];
                }
                free(split);
                free(output);
                free_ast(sub_ast);
                *pos = sub_end + 1;
            }
        } else {
            break;
        }
    }
    cmd->argv[cmd->argc] = NULL;
    return cmd;
}

static ASTNode *parse_pipeline(Token *tokens, int *pos, int end) {
    Pipeline *p = malloc(sizeof(Pipeline));
    if (!p) { perror("malloc"); exit(1); }
    p->cmds = malloc(sizeof(Command*) * MAX_CMDS);
    if (!p->cmds) { perror("malloc"); exit(1); }
    p->count = 0;
    p->cmds[p->count++] = parse_command(tokens, pos, end);
    while (*pos < end && tokens[*pos].type == TOKEN_PIPE) {
        (*pos)++;
        if (p->count >= MAX_CMDS) {
            fprintf(stderr, "pipeline too long\n");
            break;
        }
        p->cmds[p->count++] = parse_command(tokens, pos, end);
    }
    ASTNode *node = malloc(sizeof(ASTNode));
    node->type = NODE_PIPELINE;
    node->data.pipeline = p;
    return node;
}

static ASTNode *parse_if(Token *tokens, int *pos, int end) {
    (*pos)++;
    ASTNode *cond = parse_toplevel(tokens, pos, end);
    if (*pos >= end || tokens[*pos].type != TOKEN_THEN) {
        fprintf(stderr, "parse error: expected 'then'\n");
        exit(1);
    }
    (*pos)++;
    ASTNode *then_body = parse_toplevel(tokens, pos, end);
    ASTNode *else_body = NULL;
    if (*pos < end && tokens[*pos].type == TOKEN_ELSE) {
        (*pos)++;
        else_body = parse_toplevel(tokens, pos, end);
    }
    if (*pos >= end || tokens[*pos].type != TOKEN_FI) {
        fprintf(stderr, "parse error: expected 'fi'\n");
        exit(1);
    }
    (*pos)++;
    ASTNode *node = malloc(sizeof(ASTNode));
    node->type = NODE_IF;
    node->data.if_stmt.condition = cond;
    node->data.if_stmt.then_body = then_body;
    node->data.if_stmt.else_body = else_body;
    return node;
}

static ASTNode *parse_while(Token *tokens, int *pos, int end) {
    (*pos)++;
    ASTNode *cond = parse_toplevel(tokens, pos, end);
    if (*pos >= end || tokens[*pos].type != TOKEN_DO) {
        fprintf(stderr, "parse error: expected 'do'\n");
        exit(1);
    }
    (*pos)++;
    ASTNode *body = parse_toplevel(tokens, pos, end);
    if (*pos >= end || tokens[*pos].type != TOKEN_DONE) {
        fprintf(stderr, "parse error: expected 'done'\n");
        exit(1);
    }
    (*pos)++;
    ASTNode *node = malloc(sizeof(ASTNode));
    node->type = NODE_WHILE;
    node->data.while_stmt.condition = cond;
    node->data.while_stmt.body = body;
    return node;
}

static ASTNode *parse_for(Token *tokens, int *pos, int end) {
    (*pos)++;
    if (*pos >= end || tokens[*pos].type != TOKEN_WORD) {
        fprintf(stderr, "parse error: expected variable name\n");
        exit(1);
    }
    char *var = strdup(tokens[*pos].value);
    (*pos)++;
    char **wordlist = NULL;
    int wordcount = 0;
    if (*pos < end && tokens[*pos].type == TOKEN_IN) {
        (*pos)++;
        while (*pos < end && tokens[*pos].type == TOKEN_WORD) {
            wordlist = realloc(wordlist, sizeof(char*) * (wordcount+1));
            wordlist[wordcount++] = strdup(tokens[*pos].value);
            (*pos)++;
        }
    }
    if (*pos >= end || tokens[*pos].type != TOKEN_DO) {
        fprintf(stderr, "parse error: expected 'do'\n");
        exit(1);
    }
    (*pos)++;
    ASTNode *body = parse_toplevel(tokens, pos, end);
    if (*pos >= end || tokens[*pos].type != TOKEN_DONE) {
        fprintf(stderr, "parse error: expected 'done'\n");
        exit(1);
    }
    (*pos)++;
    ASTNode *node = malloc(sizeof(ASTNode));
    node->type = NODE_FOR;
    node->data.for_stmt.var = var;
    node->data.for_stmt.wordlist = wordlist;
    node->data.for_stmt.wordcount = wordcount;
    node->data.for_stmt.body = body;
    return node;
}

static ASTNode *parse_group(Token *tokens, int *pos, int end) {
    (*pos)++;
    ASTNode *body = parse_toplevel(tokens, pos, end);
    if (*pos >= end || tokens[*pos].type != TOKEN_GROUP_CLOSE) {
        fprintf(stderr, "parse error: expected ')'\n");
        exit(1);
    }
    (*pos)++;
    ASTNode *node = malloc(sizeof(ASTNode));
    node->type = NODE_GROUP;
    node->data.group.body = body;
    return node;
}

ASTNode *parse_toplevel(Token *tokens, int *pos, int end) {
    ASTNode *left = NULL;
    if (*pos >= end) return NULL;
    if (tokens[*pos].type == TOKEN_IF) {
        left = parse_if(tokens, pos, end);
    } else if (tokens[*pos].type == TOKEN_WHILE) {
        left = parse_while(tokens, pos, end);
    } else if (tokens[*pos].type == TOKEN_FOR) {
        left = parse_for(tokens, pos, end);
    } else if (tokens[*pos].type == TOKEN_GROUP_OPEN) {
        left = parse_group(tokens, pos, end);
    } else {
        left = parse_pipeline(tokens, pos, end);
    }
    while (*pos < end) {
        TokenType op = tokens[*pos].type;
        if (op == TOKEN_AND || op == TOKEN_OR || op == TOKEN_SEMICOLON) {
            (*pos)++;
            ASTNode *right = NULL;
            if (*pos < end) {
                if (tokens[*pos].type == TOKEN_IF) right = parse_if(tokens, pos, end);
                else if (tokens[*pos].type == TOKEN_WHILE) right = parse_while(tokens, pos, end);
                else if (tokens[*pos].type == TOKEN_FOR) right = parse_for(tokens, pos, end);
                else if (tokens[*pos].type == TOKEN_GROUP_OPEN) right = parse_group(tokens, pos, end);
                else right = parse_pipeline(tokens, pos, end);
            }
            ASTNode *node = malloc(sizeof(ASTNode));
            node->type = (op == TOKEN_AND) ? NODE_AND : (op == TOKEN_OR) ? NODE_OR : NODE_SEMICOLON;
            node->data.binary.left = left;
            node->data.binary.right = right;
            left = node;
        } else break;
    }
    return left;
}