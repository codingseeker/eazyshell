#ifndef BUILTINS_H
#define BUILTINS_H

#include "ast.h"

int is_builtin(Command *cmd);
int do_cd(Command *cmd);
int do_pwd(Command *cmd);
int do_export(Command *cmd);
int do_unset(Command *cmd);
int do_echo(Command *cmd);
int do_alias(Command *cmd);
int do_type(Command *cmd);
int do_fg(Command *cmd);
int do_bg(Command *cmd);
int do_jobs(Command *cmd);

#endif