#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include "expansion.h"

char *ifs = " \t\n";

char *expand_param(const char *src) {
    char *result = malloc(1);
    if (!result) return NULL;
    result[0] = '\0';
    const char *p = src;
    while (*p) {
        if (*p == '$' && *(p+1) == '{') {
            p += 2;
            char varname[256];
            int i = 0;
            while (*p && *p != '}' && *p != ':' && *p != '#' && *p != '%' && *p != '-' && *p != '=' && *p != '?' && *p != '+') {
                varname[i++] = *p++;
            }
            varname[i] = '\0';
            char op = *p;
            char *pattern = NULL;
            if (op == ':' || op == '#' || op == '%' || op == '-' || op == '=' || op == '?' || op == '+') {
                p++;
                if (*p == '-') { op = ':'; p++; }
                const char *pat_start = p;
                while (*p && *p != '}') p++;
                pattern = strndup(pat_start, p - pat_start);
            }
            if (*p == '}') p++;
            char *val = getenv(varname);
            char *ins = NULL;
            if (op == 0) {
                ins = val ? strdup(val) : strdup("");
            } else if (op == '-') {
                ins = val ? strdup(val) : strdup(pattern ? pattern : "");
            } else if (op == ':') {
                ins = (val && val[0]) ? strdup(val) : strdup(pattern ? pattern : "");
            } else if (op == '#') {
                if (val) {
                    size_t plen = pattern ? strlen(pattern) : 0;
                    if (strncmp(val, pattern, plen) == 0)
                        ins = strdup(val + plen);
                    else
                        ins = strdup(val);
                } else ins = strdup("");
            } else ins = strdup("");
            if (pattern) free(pattern);
            size_t newlen = strlen(result) + strlen(ins) + 1;
            char *tmp = realloc(result, newlen);
            if (!tmp) { perror("realloc"); exit(1); }
            result = tmp;
            strcat(result, ins);
            free(ins);
        } else {
            size_t newlen = strlen(result) + 2;
            char *tmp = realloc(result, newlen);
            if (!tmp) { perror("realloc"); exit(1); }
            result = tmp;
            char tmp2[2] = {*p, '\0'};
            strcat(result, tmp2);
            p++;
        }
    }
    return result;
}

char **split_words(const char *str, int *count) {
    char **result = NULL;
    int cnt = 0;
    char *s = strdup(str);
    char *saveptr;
    char *token = strtok_r(s, ifs, &saveptr);
    while (token) {
        result = realloc(result, sizeof(char*) * (cnt+1));
        result[cnt++] = strdup(token);
        token = strtok_r(NULL, ifs, &saveptr);
    }
    free(s);
    *count = cnt;
    return result;
}

char **expand_braces(const char *word, int *count) {
    char *open = strchr(word, '{');
    if (!open) {
        char **res = malloc(sizeof(char*));
        res[0] = strdup(word);
        *count = 1;
        return res;
    }
    char *close = strchr(open, '}');
    if (!close) {
        char **res = malloc(sizeof(char*));
        res[0] = strdup(word);
        *count = 1;
        return res;
    }
    char *inside = strndup(open+1, close - open - 1);
    char *prefix = strndup(word, open - word);
    char *suffix = strdup(close+1);
    char **res = NULL;
    int cnt = 0;
    if (strchr(inside, ',')) {
        char *saveptr;
        char *part = strtok_r(inside, ",", &saveptr);
        while (part) {
            char *full = malloc(strlen(prefix) + strlen(part) + strlen(suffix) + 1);
            strcpy(full, prefix);
            strcat(full, part);
            strcat(full, suffix);
            int subcnt;
            char **sub = expand_braces(full, &subcnt);
            for (int i = 0; i < subcnt; i++) {
                res = realloc(res, sizeof(char*) * (cnt+1));
                res[cnt++] = sub[i];
            }
            free(sub);
            free(full);
            part = strtok_r(NULL, ",", &saveptr);
        }
    } else if (strstr(inside, "..")) {
        int start, end;
        if (sscanf(inside, "%d..%d", &start, &end) == 2) {
            int step = (start <= end) ? 1 : -1;
            for (int i = start; i != end + step; i += step) {
                char num[32];
                snprintf(num, sizeof(num), "%d", i);
                char *full = malloc(strlen(prefix) + strlen(num) + strlen(suffix) + 1);
                strcpy(full, prefix);
                strcat(full, num);
                strcat(full, suffix);
                int subcnt;
                char **sub = expand_braces(full, &subcnt);
                for (int j = 0; j < subcnt; j++) {
                    res = realloc(res, sizeof(char*) * (cnt+1));
                    res[cnt++] = sub[j];
                }
                free(sub);
                free(full);
            }
        } else {
            int subcnt;
            char **sub = expand_braces(inside, &subcnt);
            for (int j = 0; j < subcnt; j++) {
                char *full = malloc(strlen(prefix) + strlen(sub[j]) + strlen(suffix) + 1);
                strcpy(full, prefix);
                strcat(full, sub[j]);
                strcat(full, suffix);
                res = realloc(res, sizeof(char*) * (cnt+1));
                res[cnt++] = full;
                free(sub[j]);
            }
            free(sub);
        }
    } else {
        int subcnt;
        char **sub = expand_braces(inside, &subcnt);
        for (int j = 0; j < subcnt; j++) {
            char *full = malloc(strlen(prefix) + strlen(sub[j]) + strlen(suffix) + 1);
            strcpy(full, prefix);
            strcat(full, sub[j]);
            strcat(full, suffix);
            res = realloc(res, sizeof(char*) * (cnt+1));
            res[cnt++] = full;
            free(sub[j]);
        }
        free(sub);
    }
    free(prefix);
    free(suffix);
    free(inside);
    *count = cnt;
    return res;
}

/* Arithmetic parser */
static const char *arith_p;
static char arith_error;

static long arith_primary(void) {
    long val = 0;
    while (isspace(*arith_p)) arith_p++;
    if (*arith_p == '(') {
        arith_p++;
        val = arith_expr();
        while (isspace(*arith_p)) arith_p++;
        if (*arith_p != ')') { arith_error = 1; return 0; }
        arith_p++;
        return val;
    }
    if (*arith_p == '+' || *arith_p == '-') {
        int sign = (*arith_p == '-') ? -1 : 1;
        arith_p++;
        val = arith_primary();
        return sign * val;
    }
    if (isalpha(*arith_p) || *arith_p == '_') {
        char varname[256];
        int i = 0;
        while (isalnum(*arith_p) || *arith_p == '_') varname[i++] = *arith_p++;
        varname[i] = '\0';
        char *env = getenv(varname);
        val = env ? atol(env) : 0;
        return val;
    }
    if (isdigit(*arith_p)) {
        char *end;
        val = strtol(arith_p, &end, 0);
        arith_p = end;
        return val;
    }
    arith_error = 1;
    return 0;
}

static long arith_factor(void) {
    long val = arith_primary();
    while (1) {
        while (isspace(*arith_p)) arith_p++;
        char op = *arith_p;
        if (op != '*' && op != '/' && op != '%') break;
        arith_p++;
        long rhs = arith_primary();
        if (op == '*') val *= rhs;
        else if (op == '/') {
            if (rhs == 0) { arith_error = 1; return 0; }
            val /= rhs;
        }
        else {
            if (rhs == 0) { arith_error = 1; return 0; }
            val %= rhs;
        }
    }
    return val;
}

static long arith_expr(void) {
    long val = arith_factor();
    while (1) {
        while (isspace(*arith_p)) arith_p++;
        char op = *arith_p;
        if (op != '+' && op != '-') break;
        arith_p++;
        long rhs = arith_factor();
        if (op == '+') val += rhs;
        else val -= rhs;
    }
    return val;
}

long eval_arith_expr(const char *expr) {
    arith_p = expr;
    arith_error = 0;
    long res = arith_expr();
    while (isspace(*arith_p)) arith_p++;
    if (arith_error || *arith_p != '\0') {
        fprintf(stderr, "arithmetic syntax error\n");
        return 0;
    }
    return res;
}