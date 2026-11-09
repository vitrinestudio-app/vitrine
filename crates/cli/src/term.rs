//! Terminal styling: plain ANSI escapes, only on a TTY and only when
//! `NO_COLOR` is unset (<https://no-color.org>). `CLICOLOR_FORCE` forces
//! colours on.

use std::io::IsTerminal;

use arrangia_core::validate::Severity;

#[derive(Debug, Clone, Copy)]
pub struct Style {
    color: bool,
}

fn env_set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| !v.is_empty() && v != "0")
}

fn use_color(tty: bool) -> bool {
    if env_set("NO_COLOR") {
        return false;
    }
    if env_set("CLICOLOR_FORCE") {
        return true;
    }
    tty && std::env::var_os("TERM").is_none_or(|t| t != "dumb")
}

impl Style {
    pub fn stdout() -> Self {
        Self {
            color: use_color(std::io::stdout().is_terminal()),
        }
    }

    pub fn stderr() -> Self {
        Self {
            color: use_color(std::io::stderr().is_terminal()),
        }
    }

    fn paint(self, sgr: &str, s: &str) -> String {
        if self.color {
            format!("\x1b[{sgr}m{s}\x1b[0m")
        } else {
            s.to_owned()
        }
    }

    pub fn bold(self, s: &str) -> String {
        self.paint("1", s)
    }

    pub fn dim(self, s: &str) -> String {
        self.paint("2", s)
    }

    pub fn ok(self, s: &str) -> String {
        self.paint("1;32", s)
    }

    pub fn error(self, s: &str) -> String {
        self.paint("1;31", s)
    }

    pub fn warn(self, s: &str) -> String {
        self.paint("1;33", s)
    }

    pub fn info(self, s: &str) -> String {
        self.paint("1;34", s)
    }

    /// `error` / `warn ` / `info ` (padded to 5), coloured by level.
    pub fn level(self, level: Severity) -> String {
        match level {
            Severity::Error => self.error("error"),
            Severity::Warn => self.warn("warn "),
            Severity::Info => self.info("info "),
        }
    }
}

/// `n thing` / `n things`.
pub fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}
