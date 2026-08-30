use std::io::Write;

use crate::error::ShellError;
use crate::shell::Shell;

const DEFAULT_IFS: &str = " \t\n";

struct Expanded {
    text: String,
    quoted: Vec<bool>,
}

pub fn expand_word(raw: &str, shell: &Shell) -> Result<Vec<String>, ShellError> {
    expand_word_globbing(raw, shell, true)
}

pub fn expand_cmd_word(raw: &str, shell: &Shell) -> Result<Vec<String>, ShellError> {
    expand_word_globbing(raw, shell, false)
}

fn expand_word_globbing(
    raw: &str,
    shell: &Shell,
    do_glob: bool,
) -> Result<Vec<String>, ShellError> {
    let variants = brace_expand(raw);
    let mut fields: Vec<String> = Vec::new();

    for variant in variants {
        let expanded = expand_variant(&variant, shell)?;
        let split = field_split(&expanded);
        for field in split {
            if field.quoted.iter().all(|&q| q) || !do_glob {
                fields.push(field.text);
            } else if field.text.contains('*')
                || field.text.contains('?')
                || field.text.contains('[')
            {
                if let Some(mut matches) = glob_expand(&field.text) {
                    if matches.is_empty() {
                        fields.push(field.text);
                    } else {
                        fields.append(&mut matches);
                    }
                } else {
                    fields.push(field.text);
                }
            } else {
                fields.push(field.text);
            }
        }
    }

    Ok(fields)
}

fn is_ifs(c: char) -> bool {
    DEFAULT_IFS.contains(c)
}

fn command_substitute(inner: &str, shell: &Shell) -> Result<String, ShellError> {
    let exe = std::env::current_exe()
        .map_err(|e| ShellError::eval(format!("command substitution: {}", e)))?;
    let mut cmd = std::process::Command::new(exe);
    cmd.env_clear();
    for (k, v) in &shell.variables {
        cmd.env(k, v);
    }
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| ShellError::eval(format!("command substitution: {}", e)))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(inner.as_bytes());
        let _ = stdin.flush();
    }

    let output = child
        .wait_with_output()
        .map_err(|e| ShellError::eval(format!("command substitution: {}", e)))?;

    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let trimmed = text.trim_end_matches(['\n', '\r']);
    Ok(trimmed.to_string())
}

fn expand_variant(raw: &str, shell: &Shell) -> Result<Expanded, ShellError> {
    let chars: Vec<char> = raw.chars().collect();
    let n = chars.len();
    let mut text = String::new();
    let mut quoted = Vec::new();
    let mut i = 0usize;

    while i < n {
        let c = chars[i];

        if c == '\'' {
            i += 1;
            while i < n && chars[i] != '\'' {
                text.push(chars[i]);
                quoted.push(true);
                i += 1;
            }
            if i >= n {
                return Err(ShellError::eval("unclosed single quote"));
            }
            i += 1;
            continue;
        }

        if c == '"' {
            i += 1;
            while i < n && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < n && matches!(chars[i + 1], '$' | '\\' | '"') {
                    text.push(chars[i + 1]);
                    quoted.push(true);
                    i += 2;
                    continue;
                }
                if chars[i] == '$' {
                    let consumed = expand_dollar(&chars, i, shell, &mut text, &mut quoted, true)?;
                    if consumed == 0 {
                        text.push('$');
                        quoted.push(true);
                        i += 1;
                    } else {
                        i += consumed;
                    }
                    continue;
                }
                text.push(chars[i]);
                quoted.push(true);
                i += 1;
            }
            if i >= n {
                return Err(ShellError::eval("unclosed double quote"));
            }
            i += 1;
            continue;
        }

        if c == '\\' && i + 1 < n {
            text.push(chars[i + 1]);
            quoted.push(true);
            i += 2;
            continue;
        }

        if c == '$' {
            let consumed = expand_dollar(&chars, i, shell, &mut text, &mut quoted, false)?;
            if consumed == 0 {
                text.push('$');
                quoted.push(false);
                i += 1;
            } else {
                i += consumed;
            }
            continue;
        }

        text.push(c);
        quoted.push(false);
        i += 1;
    }

    Ok(Expanded { text, quoted })
}

fn expand_dollar(
    chars: &[char],
    start: usize,
    shell: &Shell,
    text: &mut String,
    quoted_out: &mut Vec<bool>,
    in_dquote: bool,
) -> Result<usize, ShellError> {
    let n = chars.len();

    if start + 1 < n && chars[start + 1] == '(' {
        if start + 2 < n && chars[start + 2] == '(' {
            let mut depth = 1usize;
            let mut i = start + 3;
            while i < n {
                match chars[i] {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            if i >= n {
                return Err(ShellError::eval("unfinished arithmetic expansion"));
            }
            let expr: String = chars[start + 3..i].iter().collect();
            let value = eval_arith(&expr, shell)?;
            text.push_str(&value.to_string());
            let vlen = value.to_string().len();
            if in_dquote {
                quoted_out.extend(std::iter::repeat_n(true, vlen));
            } else {
                quoted_out.extend(std::iter::repeat_n(false, vlen));
            }
            return Ok(i - start + 2);
        }

        let mut depth = 1usize;
        let mut i = start + 2;
        while i < n {
            match chars[i] {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if i >= n {
            return Err(ShellError::eval("unfinished command substitution"));
        }
        let inner: String = chars[start + 2..i].iter().collect();
        let value = command_substitute(&inner, shell)?;
        text.push_str(&value);
        let vlen = value.len();
        if in_dquote {
            quoted_out.extend(std::iter::repeat_n(true, vlen));
        } else {
            quoted_out.extend(std::iter::repeat_n(false, vlen));
        }
        return Ok(i - start + 1);
    }

    if start + 1 < n && chars[start + 1] == '{' {
        let mut depth = 1usize;
        let mut i = start + 2;
        while i < n {
            match chars[i] {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if i >= n {
            return Err(ShellError::eval("unclosed ${"));
        }
        let name: String = chars[start + 2..i].iter().collect();
        let value = param_value(&name, shell);
        text.push_str(&value);
        let vlen = value.len();
        if in_dquote {
            quoted_out.extend(std::iter::repeat_n(true, vlen));
        } else {
            quoted_out.extend(std::iter::repeat_n(false, vlen));
        }
        return Ok(i - start + 1);
    }

    if start + 1 < n && chars[start + 1] == '?' {
        let value = shell.last_status.to_string();
        text.push_str(&value);
        let vlen = value.len();
        if in_dquote {
            quoted_out.extend(std::iter::repeat_n(true, vlen));
        } else {
            quoted_out.extend(std::iter::repeat_n(false, vlen));
        }
        return Ok(2);
    }

    if start + 1 < n && chars[start + 1] == '!' {
        let value = shell.last_bg_pid.map(|p| p.to_string()).unwrap_or_default();
        text.push_str(&value);
        let vlen = value.len();
        if in_dquote {
            quoted_out.extend(std::iter::repeat_n(true, vlen));
        } else {
            quoted_out.extend(std::iter::repeat_n(false, vlen));
        }
        return Ok(2);
    }

    if start + 1 < n {
        let c = chars[start + 1];
        let name_len = if c.is_ascii_digit() {
            1
        } else if c.is_ascii_alphabetic() || c == '_' {
            read_ident_len(chars, start + 1)
        } else {
            return Ok(0);
        };
        let name: String = chars[start + 1..start + 1 + name_len].iter().collect();
        let value = param_value(&name, shell);
        text.push_str(&value);
        let vlen = value.len();
        if in_dquote {
            quoted_out.extend(std::iter::repeat_n(true, vlen));
        } else {
            quoted_out.extend(std::iter::repeat_n(false, vlen));
        }
        return Ok(1 + name_len);
    }

    Ok(0)
}

fn read_ident_len(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
        i += 1;
    }
    i - start
}

fn param_value(name: &str, shell: &Shell) -> String {
    match name {
        "?" => shell.last_status.to_string(),
        "!" => shell.last_bg_pid.map(|p| p.to_string()).unwrap_or_default(),
        _ => shell
            .get_var(name)
            .map(|s| s.to_string())
            .unwrap_or_default(),
    }
}

fn field_split(expanded: &Expanded) -> Vec<Expanded> {
    let mut fields: Vec<Expanded> = Vec::new();
    let mut cur_text = String::new();
    let mut cur_quoted = Vec::new();
    let mut at_field_start = true;

    let chars: Vec<char> = expanded.text.chars().collect();
    let mut qiter = expanded.quoted.iter();

    let n = chars.len();
    let mut i = 0usize;

    while i < n {
        let c = chars[i];
        let is_q = qiter.next().copied().unwrap_or(false);

        if !is_q && is_ifs(c) {
            if !at_field_start {
                fields.push(Expanded {
                    text: cur_text,
                    quoted: cur_quoted,
                });
                cur_text = String::new();
                cur_quoted = Vec::new();
                at_field_start = true;
            }
            i += 1;
            continue;
        }

        cur_text.push(c);
        cur_quoted.push(is_q);
        at_field_start = false;
        i += 1;
    }

    if !at_field_start {
        fields.push(Expanded {
            text: cur_text,
            quoted: cur_quoted,
        });
    } else if fields.is_empty() {
        fields.push(Expanded {
            text: String::new(),
            quoted: Vec::new(),
        });
    }

    fields
}

fn brace_expand(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    brace_expand_into(raw, &mut out);
    out
}

fn brace_expand_into(raw: &str, out: &mut Vec<String>) {
    let chars: Vec<char> = raw.chars().collect();

    let open = find_unquoted_char(&chars, '{');
    let open = match open {
        Some(i) => i,
        None => {
            out.push(raw.to_string());
            return;
        }
    };

    let close = match find_matching_brace(&chars, open) {
        Some(i) => i,
        None => {
            out.push(raw.to_string());
            return;
        }
    };

    let prefix: String = chars[..open].iter().collect();
    let inner: String = chars[open + 1..close].iter().collect();
    let tail: String = chars[close + 1..].iter().collect();

    let alternatives = split_top_commas(&inner);
    let expanded_alts = if alternatives.len() == 2 {
        match expand_range(&alternatives[0], &alternatives[1]) {
            Some(range) => range,
            None => alternatives,
        }
    } else if alternatives.len() == 1 {
        match split_once_dots(&alternatives[0]) {
            Some((a, b)) => expand_range(a, b).unwrap_or(alternatives),
            None => alternatives,
        }
    } else {
        alternatives
    };

    for alt in expanded_alts {
        let rebuilt = format!("{}{}{}", prefix, alt, tail);
        brace_expand_into(&rebuilt, out);
    }
}

fn find_unquoted_char(chars: &[char], target: char) -> Option<usize> {
    let mut in_squote = false;
    let mut in_dquote = false;
    let mut escaped = false;
    for (i, &c) in chars.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' && !in_squote {
            escaped = true;
            continue;
        }
        if c == '\'' && !in_dquote {
            in_squote = !in_squote;
            continue;
        }
        if c == '"' && !in_squote {
            in_dquote = !in_dquote;
            continue;
        }
        if !in_squote && !in_dquote && c == target {
            if target == '{' && i > 0 && chars[i - 1] == '$' {
                continue;
            }
            return Some(i);
        }
    }
    None
}

fn find_matching_brace(chars: &[char], open: usize) -> Option<usize> {
    let mut in_squote = false;
    let mut in_dquote = false;
    let mut escaped = false;
    let mut depth = 0usize;
    for i in open..chars.len() {
        let c = chars[i];
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' && !in_squote {
            escaped = true;
            continue;
        }
        if c == '\'' && !in_dquote {
            in_squote = !in_squote;
            continue;
        }
        if c == '"' && !in_squote {
            in_dquote = !in_dquote;
            continue;
        }
        if !in_squote && !in_dquote {
            if c == '{' && i > 0 && chars[i - 1] == '$' {
                continue;
            }
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
    }
    None
}

fn split_top_commas(inner: &str) -> Vec<String> {
    let chars: Vec<char> = inner.chars().collect();
    let mut parts = Vec::new();
    let mut in_squote = false;
    let mut in_dquote = false;
    let mut escaped = false;
    let mut depth = 0usize;
    let mut start = 0usize;

    for (i, &c) in chars.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' && !in_squote {
            escaped = true;
            continue;
        }
        if c == '\'' && !in_dquote {
            in_squote = !in_squote;
            continue;
        }
        if c == '"' && !in_squote {
            in_dquote = !in_dquote;
            continue;
        }
        if !in_squote && !in_dquote {
            if c == '{' && i > 0 && chars[i - 1] == '$' {
                continue;
            }
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
            } else if c == ',' && depth == 0 {
                parts.push(chars[start..i].iter().collect());
                start = i + 1;
            }
        }
    }
    parts.push(chars[start..].iter().collect());
    parts
}

fn split_once_dots(s: &str) -> Option<(&str, &str)> {
    let idx = s.find("..")?;
    Some((&s[..idx], &s[idx + 2..]))
}

fn expand_range(first: &str, last: &str) -> Option<Vec<String>> {
    const MAX_BRACE_RANGE: i64 = 100_000;
    if let (Ok(a), Ok(b)) = (first.parse::<i64>(), last.parse::<i64>()) {
        let count = b.saturating_sub(a).saturating_abs();
        if count >= MAX_BRACE_RANGE {
            return None;
        }
        let mut out = Vec::new();
        if a <= b {
            let mut v = a;
            while v <= b {
                out.push(v.to_string());
                v += 1;
            }
        } else {
            let mut v = a;
            while v >= b {
                out.push(v.to_string());
                v -= 1;
            }
        }
        return Some(out);
    }

    if first.len() == 1 && last.len() == 1 {
        let a = first.chars().next().unwrap();
        let b = last.chars().next().unwrap();
        if a.is_ascii_alphabetic()
            && b.is_ascii_alphabetic()
            && a <= b
            && a.is_ascii_lowercase() == b.is_ascii_lowercase()
        {
            return Some((a..=b).map(|c| c.to_string()).collect());
        }
    }

    None
}

fn glob_expand(pattern: &str) -> Option<Vec<String>> {
    if !has_meta(pattern) {
        return None;
    }

    let sep = '/';
    let parts: Vec<&str> = pattern.split(sep).collect();
    let has_leading_slash = pattern.starts_with('/');

    let mut current: Vec<String> = vec![if has_leading_slash {
        "/".to_string()
    } else {
        String::new()
    }];

    for (idx, part) in parts.iter().enumerate() {
        if idx == 0 && part.is_empty() {
            continue;
        }
        let mut next: Vec<String> = Vec::new();
        for base in &current {
            if has_meta(part) {
                let base = if base.is_empty() {
                    ".".to_string()
                } else if base == "/" {
                    "/".to_string()
                } else {
                    base.clone()
                };
                let entries = list_dir(&base);
                for e in entries {
                    if glob_match(part, &e) {
                        let joined = if base == "/" {
                            format!("/{}", e)
                        } else if base == "." {
                            e.clone()
                        } else {
                            format!("{}/{}", base, e)
                        };
                        next.push(joined);
                    }
                }
            } else {
                let joined = if base.is_empty() {
                    part.to_string()
                } else if base == "/" {
                    format!("/{}", part)
                } else {
                    format!("{}/{}", base, part)
                };
                next.push(joined);
            }
        }
        current = next;
    }

    let result: Vec<String> = current.into_iter().filter(|p| p != ".").collect();
    if result.is_empty() {
        Some(Vec::new())
    } else {
        let mut sorted = result;
        sorted.sort();
        Some(sorted)
    }
}

fn has_meta(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

fn list_dir(dir: &str) -> Vec<String> {
    match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_match_inner(&p, &t)
}

fn glob_match_inner(p: &[char], t: &[char]) -> bool {
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star_pi = None;
    let mut star_ti = 0usize;

    while ti < t.len() {
        if pi < p.len() {
            match p[pi] {
                '*' => {
                    star_pi = Some(pi);
                    star_ti = ti;
                    pi += 1;
                    continue;
                }
                '?' => {
                    pi += 1;
                    ti += 1;
                    continue;
                }
                '[' => {
                    let (matched, next_pi) = match_class(p, pi, t[ti]);
                    if matched {
                        pi = next_pi;
                        ti += 1;
                        continue;
                    }
                }
                c => {
                    if c == t[ti] {
                        pi += 1;
                        ti += 1;
                        continue;
                    }
                }
            }
        }

        if let Some(sp) = star_pi {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
            continue;
        }

        return false;
    }

    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }

    pi == p.len()
}

fn match_class(p: &[char], start: usize, c: char) -> (bool, usize) {
    let mut i = start + 1;
    let negate = i < p.len() && (p[i] == '!' || p[i] == '^');
    if negate {
        i += 1;
    }
    let mut matched = false;
    let mut first = true;
    while i < p.len() && (first || p[i] != ']') {
        first = false;
        if i + 2 < p.len() && p[i + 1] == '-' && p[i + 2] != ']' {
            let lo = p[i];
            let hi = p[i + 2];
            if c >= lo && c <= hi {
                matched = true;
            }
            i += 3;
            continue;
        }
        if p[i] == c {
            matched = true;
        }
        i += 1;
    }
    if i >= p.len() {
        return (true, p.len());
    }
    i += 1;
    if negate {
        matched = !matched;
    }
    (matched, i)
}

fn eval_arith(expr: &str, shell: &Shell) -> Result<i64, ShellError> {
    let mut parser = ArithParser {
        chars: expr.chars().collect(),
        pos: 0,
        shell,
    };
    let v = parser.parse_expr()?;
    parser.skip_ws();
    if parser.peek().is_some() {
        return Err(ShellError::eval("bad arithmetic syntax"));
    }
    Ok(v)
}

struct ArithParser<'a> {
    chars: Vec<char>,
    pos: usize,
    shell: &'a Shell,
}

impl<'a> ArithParser<'a> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ') | Some('\t') | Some('\n')) {
            self.pos += 1;
        }
    }

    fn parse_expr(&mut self) -> Result<i64, ShellError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<i64, ShellError> {
        let mut left = self.parse_and()?;
        loop {
            self.skip_ws();
            if self.peek() == Some('|') && self.chars.get(self.pos + 1) == Some(&'|') {
                self.pos += 2;
                let right = self.parse_and()?;
                left = if left != 0 || right != 0 { 1 } else { 0 };
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<i64, ShellError> {
        let mut left = self.parse_cmp()?;
        loop {
            self.skip_ws();
            if self.peek() == Some('&') && self.chars.get(self.pos + 1) == Some(&'&') {
                self.pos += 2;
                let right = self.parse_cmp()?;
                left = if left != 0 && right != 0 { 1 } else { 0 };
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_cmp(&mut self) -> Result<i64, ShellError> {
        let mut left = self.parse_add()?;
        loop {
            self.skip_ws();
            let op = match (self.peek(), self.chars.get(self.pos + 1).copied()) {
                (Some('='), Some('=')) => Some("=="),
                (Some('!'), Some('=')) => Some("!="),
                (Some('<'), Some('=')) => Some("<="),
                (Some('>'), Some('=')) => Some(">="),
                (Some('<'), _) => Some("<"),
                (Some('>'), _) => Some(">"),
                _ => None,
            };
            match op {
                Some("==") => {
                    self.pos += 2;
                    let r = self.parse_add()?;
                    left = (left == r) as i64;
                }
                Some("!=") => {
                    self.pos += 2;
                    let r = self.parse_add()?;
                    left = (left != r) as i64;
                }
                Some("<=") => {
                    self.pos += 2;
                    let r = self.parse_add()?;
                    left = (left <= r) as i64;
                }
                Some(">=") => {
                    self.pos += 2;
                    let r = self.parse_add()?;
                    left = (left >= r) as i64;
                }
                Some("<") => {
                    self.pos += 1;
                    let r = self.parse_add()?;
                    left = (left < r) as i64;
                }
                Some(">") => {
                    self.pos += 1;
                    let r = self.parse_add()?;
                    left = (left > r) as i64;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<i64, ShellError> {
        let mut left = self.parse_mul()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('+') => {
                    self.pos += 1;
                    let r = self.parse_mul()?;
                    left = left.wrapping_add(r);
                }
                Some('-') => {
                    self.pos += 1;
                    let r = self.parse_mul()?;
                    left = left.wrapping_sub(r);
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<i64, ShellError> {
        let mut left = self.parse_unary()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('*') => {
                    self.pos += 1;
                    let r = self.parse_unary()?;
                    left = left.wrapping_mul(r);
                }
                Some('/') => {
                    self.pos += 1;
                    let r = self.parse_unary()?;
                    if r == 0 {
                        return Err(ShellError::eval("division by zero"));
                    }
                    left = left.wrapping_div(r);
                }
                Some('%') => {
                    self.pos += 1;
                    let r = self.parse_unary()?;
                    if r == 0 {
                        return Err(ShellError::eval("division by zero"));
                    }
                    left = left.wrapping_rem(r);
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<i64, ShellError> {
        self.skip_ws();
        match self.peek() {
            Some('-') => {
                self.pos += 1;
                Ok(self.parse_unary()?.wrapping_neg())
            }
            Some('+') => {
                self.pos += 1;
                self.parse_unary()
            }
            Some('!') => {
                self.pos += 1;
                let v = self.parse_unary()?;
                Ok(if v == 0 { 1 } else { 0 })
            }
            Some('(') => {
                self.pos += 1;
                let v = self.parse_expr()?;
                self.skip_ws();
                if self.peek() != Some(')') {
                    return Err(ShellError::eval("missing ')' in arithmetic"));
                }
                self.pos += 1;
                Ok(v)
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<i64, ShellError> {
        self.skip_ws();
        let c = self.peek();
        match c {
            Some(d) if d.is_ascii_digit() => {
                let start = self.pos;
                while self.peek().map(|x| x.is_ascii_digit()).unwrap_or(false) {
                    self.pos += 1;
                }
                let s: String = self.chars[start..self.pos].iter().collect();
                s.parse::<i64>().map_err(|_| ShellError::eval("bad number"))
            }
            Some(_) => {
                if c.map(|ch| ch.is_ascii_alphabetic() || ch == '_')
                    .unwrap_or(false)
                {
                    let start = self.pos;
                    while self
                        .peek()
                        .map(|x| x.is_ascii_alphanumeric() || x == '_')
                        .unwrap_or(false)
                    {
                        self.pos += 1;
                    }
                    let name: String = self.chars[start..self.pos].iter().collect();
                    let value = param_value(&name, self.shell);
                    value
                        .trim()
                        .parse::<i64>()
                        .map_err(|_| ShellError::eval(format!("unset or bad variable: {}", name)))
                } else {
                    Err(ShellError::eval("bad arithmetic expression"))
                }
            }
            None => Err(ShellError::eval("empty arithmetic expression")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell_with(vars: &[(&str, &str)]) -> Shell {
        let mut s = Shell::new();
        s.variables.clear();
        for (k, v) in vars {
            s.variables.push((k.to_string(), v.to_string()));
        }
        s
    }

    fn expand_single(raw: &str, shell: &Shell) -> Vec<String> {
        expand_word(raw, shell).unwrap()
    }

    #[test]
    fn plain_words_are_unchanged() {
        let s = shell_with(&[]);
        assert_eq!(expand_single("hello", &s), vec!["hello"]);
        assert_eq!(expand_single("-la", &s), vec!["-la"]);
    }

    #[test]
    fn simple_param_expansion() {
        let s = shell_with(&[("HOME", "/home/u")]);
        assert_eq!(expand_single("$HOME", &s), vec!["/home/u"]);
        assert_eq!(expand_single("${HOME}/x", &s), vec!["/home/u/x"]);
    }

    #[test]
    fn param_in_middle_of_word_stays_attached() {
        let s = shell_with(&[("X", "b")]);
        assert_eq!(expand_single("a${X}c", &s), vec!["abc"]);
    }

    #[test]
    fn unset_var_expands_empty() {
        let s = shell_with(&[]);
        assert_eq!(expand_single("$NOPE", &s), vec![""]);
    }

    #[test]
    fn last_status_expansion() {
        let mut s = shell_with(&[]);
        s.last_status = 42;
        assert_eq!(expand_single("$?", &s), vec!["42"]);
    }

    #[test]
    fn word_splitting_on_unquoted_ifs() {
        let s = shell_with(&[("X", "a b")]);
        assert_eq!(expand_single("$X", &s), vec!["a", "b"]);
    }

    #[test]
    fn double_quotes_prevent_splitting() {
        let s = shell_with(&[("X", "a b")]);
        assert_eq!(expand_single("\"$X\"", &s), vec!["a b"]);
    }

    #[test]
    fn single_quotes_are_literal() {
        let s = shell_with(&[("X", "y")]);
        assert_eq!(expand_single("'$X'", &s), vec!["$X"]);
        assert_eq!(expand_single("'a b'", &s), vec!["a b"]);
    }

    #[test]
    fn mixed_quoted_and_unquoted_in_one_word() {
        let s = shell_with(&[("X", "p q")]);
        assert_eq!(expand_single("pre-$X-post", &s), vec!["pre-p", "q-post"]);
    }

    #[test]
    fn arithmetic_expansion() {
        let s = shell_with(&[]);
        assert_eq!(expand_single("$(( 1 + 2 ))", &s), vec!["3"]);
        assert_eq!(expand_single("$(( 2 * 3 + 4 ))", &s), vec!["10"]);
    }

    #[test]
    fn arithmetic_with_variables() {
        let s = shell_with(&[("A", "5")]);
        assert_eq!(expand_single("$(( A * 2 ))", &s), vec!["10"]);
    }

    #[test]
    fn arithmetic_embedded_in_word() {
        let s = shell_with(&[]);
        assert_eq!(expand_single("x$((1 + 2))yy", &s), vec!["x3yy"]);
    }

    #[test]
    fn brace_expansion_comma() {
        let s = shell_with(&[]);
        assert_eq!(expand_single("{a,b,c}", &s), vec!["a", "b", "c"]);
    }

    #[test]
    fn brace_expansion_range() {
        let s = shell_with(&[]);
        assert_eq!(expand_single("f{1..3}", &s), vec!["f1", "f2", "f3"]);
    }

    #[test]
    fn brace_expansion_prefixed() {
        let s = shell_with(&[]);
        assert_eq!(expand_single("x{a,b}y", &s), vec!["xay", "xby"]);
    }

    #[test]
    fn quoted_braces_not_expanded() {
        let s = shell_with(&[]);
        assert_eq!(expand_single("'{a,b}'", &s), vec!["{a,b}"]);
    }

    #[test]
    fn bare_dollar_is_literal() {
        let s = shell_with(&[]);
        assert_eq!(expand_single("$", &s), vec!["$"]);
        assert_eq!(expand_single("a $ bc", &s), vec!["a", "$", "bc"]);
    }

    #[test]
    fn glob_no_match_stays_literal() {
        let s = shell_with(&[]);
        let fields = expand_single("zznope*", &s);
        assert_eq!(fields, vec!["zznope*"]);
    }

    #[test]
    fn glob_matcher_handles_wildcards() {
        assert!(glob_match("*.rs", "main.rs"));
        assert!(glob_match("*.rs", "foo_bar.rs"));
        assert!(!glob_match("*.rs", "main.txt"));
        assert!(glob_match("f?o", "foo"));
        assert!(!glob_match("f?o", "fooo"));
        assert!(glob_match("[abc]", "b"));
        assert!(!glob_match("[abc]", "d"));
        assert!(glob_match("a*z", "aXYZz"));
        assert!(!glob_match("a*z", "abzq"));
    }

    #[test]
    fn quote_after_expansion_prevents_globbing() {
        let s = shell_with(&[]);
        let fields = expand_single("\"*\"", &s);
        assert_eq!(fields, vec!["*"]);
    }
}
