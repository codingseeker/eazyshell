#ifndef EXECUTOR_H
#define EXECUTOR_H

#include "ast.h"

int exec_pipeline(Pipeline *p, int bg, int subshell);
int eval_ast(ASTNode *node, int bg, int subshell);

#endif
