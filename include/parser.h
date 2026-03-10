#ifndef PARSER_H
#define PARSER_H

#include "ast.h"

ASTNode *parse_toplevel(Token *tokens, int *pos, int end);

#endif