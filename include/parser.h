parser.h#ifndef PARSER_H
#define PARSER_H

#include "AST.h"
#include "lexer.h"

ASTNode *parse(Token *tokens, int count);

#endif