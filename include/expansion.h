#ifndef EXPANSION_H
#define EXPANSION_H

char *expand_param(const char *src);
char **split_words(const char *str, int *count);
char **expand_braces(const char *word, int *count);
long eval_arith_expr(const char *expr);

extern char *ifs;

#endif