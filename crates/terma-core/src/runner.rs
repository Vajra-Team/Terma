//! Externe Befehle. Alles läuft über `Runner`, damit Tests ohne echtes System auskommen.
//! Exit 127 heißt: Befehl nicht gefunden.

use std::io::{BufRead, BufReader, ErrorKind};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;

pub trait Runner: Send + Sync {
    /// Führt aus und meldet stdout und stderr zeilenweise.
    fn run(&self, argv: &[&str], env: &[(&str, &str)], cwd: Option<&Path>, on_line: &mut dyn FnMut(&str)) -> std::io::Result<i32>;
    /// Führt aus und gibt Exit-Code und stdout zurück.
    fn output(&self, argv: &[&str]) -> std::io::Result<(i32, String)>;
}

pub struct SystemRunner;

fn command(argv: &[&str]) -> Command {
    let mut cmd = Command::new(argv[0]);
    cmd.args(&argv[1..]).env("LC_ALL", "C");
    cmd
}

impl Runner for SystemRunner {
    fn run(&self, argv: &[&str], env: &[(&str, &str)], cwd: Option<&Path>, on_line: &mut dyn FnMut(&str)) -> std::io::Result<i32> {
        let mut cmd = command(argv);
        cmd.envs(env.iter().copied()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(127),
            Err(e) => return Err(e),
        };
        let (tx, rx) = mpsc::channel::<String>();
        let readers: Vec<_> = [
            child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
            child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        ]
        .into_iter()
        .flatten()
        .map(|stream| {
            let tx = tx.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            })
        })
        .collect();
        drop(tx);
        for line in rx {
            on_line(&line);
        }
        for r in readers {
            let _ = r.join();
        }
        Ok(child.wait()?.code().unwrap_or(-1))
    }

    fn output(&self, argv: &[&str]) -> std::io::Result<(i32, String)> {
        match command(argv).stdin(Stdio::null()).output() {
            Ok(out) => Ok((out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok((127, String::new())),
            Err(e) => Err(e),
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
pub use fake::{Call, FakeRunner};

#[cfg(any(test, feature = "test-util"))]
mod fake {
    use super::Runner;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    #[derive(Debug, Clone, PartialEq)]
    pub struct Call {
        pub argv: Vec<String>,
        pub env: Vec<(String, String)>,
        pub cwd: Option<PathBuf>,
    }

    /// Antwortet nach Regeln. Eine Regel greift, wenn ihre Wörter in dieser Reihenfolge im Aufruf vorkommen.
    /// Die zuerst angelegte passende Regel gewinnt. Ohne passende Regel: Exit 127.
    #[derive(Default)]
    pub struct FakeRunner {
        rules: Vec<(Vec<String>, i32, String)>,
        calls: Mutex<Vec<Call>>,
    }

    impl FakeRunner {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn on(mut self, pattern: &[&str], code: i32, stdout: &str) -> Self {
            self.rules.push((pattern.iter().map(|s| s.to_string()).collect(), code, stdout.to_string()));
            self
        }

        pub fn calls(&self) -> Vec<Call> {
            self.calls.lock().unwrap().clone()
        }

        fn answer(&self, argv: &[&str], env: &[(&str, &str)], cwd: Option<&Path>) -> (i32, String) {
            self.calls.lock().unwrap().push(Call {
                argv: argv.iter().map(|s| s.to_string()).collect(),
                env: env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
                cwd: cwd.map(Path::to_path_buf),
            });
            self.rules
                .iter()
                .find(|(pattern, _, _)| {
                    let mut rest = argv.iter();
                    pattern.iter().all(|p| rest.any(|a| a == p))
                })
                .map(|(_, code, out)| (*code, out.clone()))
                .unwrap_or((127, String::new()))
        }
    }

    impl Runner for FakeRunner {
        fn run(&self, argv: &[&str], env: &[(&str, &str)], cwd: Option<&Path>, on_line: &mut dyn FnMut(&str)) -> std::io::Result<i32> {
            let (code, out) = self.answer(argv, env, cwd);
            out.lines().for_each(|l| on_line(l));
            Ok(code)
        }

        fn output(&self, argv: &[&str]) -> std::io::Result<(i32, String)> {
            Ok(self.answer(argv, &[], None))
        }
    }
}

/// Merkt sich die letzten Zeilen einer Ausgabe für Fehlermeldungen.
#[derive(Default)]
pub struct LineTail {
    lines: std::collections::VecDeque<String>,
}

impl LineTail {
    const KEEP: usize = 20;

    pub fn push(&mut self, line: &str) {
        if self.lines.len() == Self::KEEP {
            self.lines.pop_front();
        }
        self.lines.push_back(line.to_string());
    }

    pub fn text(&self) -> String {
        self.lines.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}
