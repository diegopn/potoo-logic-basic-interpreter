use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

const CLI: &str = env!("CARGO_BIN_EXE_potoo-logic-basic");

struct ProgramDirectory(std::path::PathBuf);

impl ProgramDirectory {
    fn new() -> Self {
        let unique = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory =
            std::env::temp_dir().join(format!("potoo-logic-basic-{}-{unique}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }

    fn write(&self, name: &str, source: &str) -> std::path::PathBuf {
        let path = self.0.join(name);
        fs::write(&path, source).unwrap();
        path
    }
}

impl Drop for ProgramDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(path: &std::path::Path, input: &str) -> Output {
    let mut child = Command::new(CLI)
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn help_prints_usage_without_a_program_file() {
    let output = Command::new(CLI).arg("--help").output().unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Usage: potoo-logic-basic <program.lbc>\n"
    );
}

#[test]
fn cli_runs_a_source_file_and_prints_its_text_window() {
    let directory = ProgramDirectory::new();
    let path = directory.write("hello.lbc", "Escreva \"Olá, Logic Basic!\"\nFimPrograma");

    let output = run(&path, "");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Olá, Logic Basic!\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn cli_routes_text_input_and_shows_its_prompt_on_stderr() {
    let directory = ProgramDirectory::new();
    let path = directory.write(
        "input.lbc",
        "Memoria Nome\nNome = EntraTexto, \"Seu nome?\"\nEscreva Nome\nFimPrograma",
    );

    let output = run(&path, "Ada\n");

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "Ada\n");
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Seu nome? ")
    );
}

#[test]
fn cli_shows_the_text_window_before_key_wait_and_returns_one_character() {
    let directory = ProgramDirectory::new();
    let path = directory.write(
        "wait.lbc",
        "Memoria Tecla\nEscreva \"Pressione uma tecla\"\nAguarde Tecla\nEscreva \"Tecla: \", Tecla\nFimPrograma",
    );

    let output = run(&path, "Qabc\n");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Pressione uma tecla\nTecla: Q\n"
    );
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Pressione uma tecla")
    );
}

#[test]
fn cli_reports_the_source_line_and_exits_unsuccessfully_for_invalid_programs() {
    let directory = ProgramDirectory::new();
    let path = directory.write("invalid.lbc", "Escreva \"ok\"\nFimSe");

    let output = run(&path, "");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("line 2:"));
}

#[test]
fn cli_loads_an_included_program_relative_to_the_main_file() {
    let directory = ProgramDirectory::new();
    let path = directory.write(
        "main.lbc",
        "CarregaPrograma \"parte\"\nEscreva \"fim\"\nFimPrograma",
    );
    directory.write(
        "parte.lbc",
        "Memoria Incluida\nIncluida = 7\nEscreva Incluida",
    );

    let output = run(&path, "");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "7\nfim\n");
}
