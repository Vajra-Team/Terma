//! `.desktop`-Dateien lesen und gezielt ändern. Alles, was nicht geändert wird, bleibt Zeichen für Zeichen erhalten.

use std::fmt;
use std::path::Path;

const MAIN_GROUP: &str = "Desktop Entry";

#[derive(Debug, Clone, PartialEq)]
enum Line {
    Group(String),
    Entry { key: String, value: String, raw: Option<String> },
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DesktopEntry {
    lines: Vec<Line>,
    trailing_newline: bool,
}

impl DesktopEntry {
    pub fn parse(text: &str) -> DesktopEntry {
        let lines = text
            .lines()
            .map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with('[') && trimmed.ends_with(']') {
                    Line::Group(trimmed[1..trimmed.len() - 1].to_string())
                } else if !trimmed.starts_with('#') && line.contains('=') {
                    let (k, v) = line.split_once('=').unwrap();
                    Line::Entry { key: k.trim().to_string(), value: v.trim_start().to_string(), raw: Some(line.to_string()) }
                } else {
                    Line::Other(line.to_string())
                }
            })
            .collect();
        DesktopEntry { lines, trailing_newline: text.ends_with('\n') }
    }

    /// Bereich der Hauptgruppe: (Index der Gruppenzeile, Index hinter ihrer letzten Zeile).
    fn main_range(&self) -> Option<(usize, usize)> {
        let start = self.lines.iter().position(|l| matches!(l, Line::Group(g) if g == MAIN_GROUP))?;
        let end = self.lines[start + 1..]
            .iter()
            .position(|l| matches!(l, Line::Group(_)))
            .map(|i| start + 1 + i)
            .unwrap_or(self.lines.len());
        Some((start, end))
    }

    fn find(&self, key: &str) -> Option<usize> {
        let (start, end) = self.main_range()?;
        (start + 1..end).find(|&i| matches!(&self.lines[i], Line::Entry { key: k, .. } if k == key))
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        match &self.lines[self.find(key)?] {
            Line::Entry { value, .. } => Some(value.as_str()),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        let line = Line::Entry { key: key.to_string(), value: value.to_string(), raw: None };
        if let Some(i) = self.find(key) {
            self.lines[i] = line;
            return;
        }
        let (start, end) = match self.main_range() {
            Some(r) => r,
            None => {
                self.lines.insert(0, Line::Group(MAIN_GROUP.to_string()));
                (0, 1)
            }
        };
        // Hinter den letzten Eintrag der Gruppe, vor Leerzeilen, die zur nächsten Gruppe überleiten.
        let insert_at = (start + 1..end)
            .rev()
            .find(|&i| matches!(self.lines[i], Line::Entry { .. }))
            .map(|i| i + 1)
            .unwrap_or(start + 1);
        self.lines.insert(insert_at, line);
    }

    pub fn remove(&mut self, key: &str) {
        if let Some(i) = self.find(key) {
            self.lines.remove(i);
        }
    }

    pub fn mime_types(&self) -> Vec<String> {
        self.get("MimeType")
            .map(|v| v.split(';').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// Das Programm aus `Exec`, ohne Anführungszeichen und Escapes.
    pub fn exec_program(&self) -> Option<String> {
        let (program, _) = split_first_arg(&unescape_string(self.get("Exec")?))?;
        Some(program)
    }

    /// Ersetzt das Programm in `Exec` und behält die Argumente.
    pub fn set_exec_program(&mut self, path: &Path) {
        let rest = self
            .get("Exec")
            .and_then(|v| split_first_arg(&unescape_string(v)))
            .map(|(_, rest)| rest)
            .unwrap_or_default();
        let value = escape_string(&format!("{}{}", quote_arg(&path.to_string_lossy()), rest));
        self.set("Exec", &value);
    }
}

impl fmt::Display for DesktopEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text: Vec<String> = self
            .lines
            .iter()
            .map(|l| match l {
                Line::Group(g) => format!("[{g}]"),
                Line::Entry { raw: Some(raw), .. } => raw.clone(),
                Line::Entry { key, value, raw: None } => format!("{key}={value}"),
                Line::Other(o) => o.clone(),
            })
            .collect();
        f.write_str(&text.join("\n"))?;
        if self.trailing_newline || self.lines.iter().any(|l| matches!(l, Line::Entry { raw: None, .. })) {
            f.write_str("\n")?;
        }
        Ok(())
    }
}

/// Escapes auf Ebene des String-Typs der Desktop Entry Specification auflösen (`\\`, `\s`, `\n`, `\t`, `\r`).
fn unescape_string(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn escape_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\n', "\\n").replace('\t', "\\t").replace('\r', "\\r")
}

const RESERVED: &[char] = &[' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(', ')', '`'];

/// Quotet ein Argument für `Exec`, wenn es reservierte Zeichen enthält.
fn quote_arg(arg: &str) -> String {
    if !arg.contains(RESERVED) {
        return arg.to_string();
    }
    let mut out = String::from("\"");
    for c in arg.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// Erstes Argument (entquotet) und der Rest der Zeile ab dem Trenner, unverändert.
fn split_first_arg(exec: &str) -> Option<(String, String)> {
    let exec = exec.trim_start();
    let mut chars = exec.char_indices();
    let mut arg = String::new();
    if exec.starts_with('"') {
        chars.next();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' => {
                    if let Some((_, next)) = chars.next() {
                        arg.push(next);
                    }
                }
                '"' => return Some((arg, exec[i + 1..].to_string())),
                _ => arg.push(c),
            }
        }
        None
    } else {
        let end = exec.find(char::is_whitespace).unwrap_or(exec.len());
        if end == 0 {
            return None;
        }
        Some((exec[..end].to_string(), exec[end..].to_string()))
    }
}
