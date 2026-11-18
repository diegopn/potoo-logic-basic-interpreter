<p align="center">
  <img src="https://diegopn.github.io/potoo-site/src/assets/images/language-interpreter-icon.svg?v=1c651ca7" alt="Potoo IDE language interpreter icon" width="120" />
</p>

<h1 align="center">Logic Basic Interpreter</h1>

<p align="center">An independent Rust interpreter library and CLI for the Logic Basic programming language.</p>

<p align="center">
  <a href="https://github.com/diegopn/potoo-logic-basic-interpreter/releases/latest">Download</a> ·
  <a href="https://diegopn.github.io/potoo-site/">Website</a> ·
  <a href="https://diegopn.github.io/potoo-site/manual/logic-basic/">User manual</a>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white"></a>
  <a href="https://doc.rust-lang.org/cargo/"><img alt="Cargo" src="https://img.shields.io/badge/Cargo-DEA584?style=for-the-badge&logo=rust&logoColor=white"></a>
  <img alt="Rust library" src="https://img.shields.io/badge/Rust_library-4A86CF?style=for-the-badge">
  <img alt="CLI" src="https://img.shields.io/badge/CLI-6A4C93?style=for-the-badge">
</p>

Originally developed as part of the Potoo IDE project, this interpreter is an independent Rust software package intended to be distributed separately from the IDE. It can be embedded as a library in other Rust projects or run as a standalone command-line tool on Linux, macOS, and Windows.

Logic Basic is a programming language, not a pseudolanguage. This interpreter implements the documented 7.0 command language; its host API lets Potoo IDE provide the visual interface.

The available introductory manual mentions a database function named `Entre`, but does not define its call syntax or argument rules. That function is not included in the implemented subset.

## Original language references

- [Curso Básico de Logic Basic 7.0, Eleusmário Mariano Rabelo](https://pt.scribd.com/document/420235581/Manual-do-Logic-Basic) — primary reference for the documented language.
- [Potoo IDE Logic Basic manual](https://diegopn.github.io/potoo-site/manual/logic-basic/) — supplemental reference for using Logic Basic in Potoo IDE.

## Source example

Save this program in a UTF-8 file named `saudacao.lbc`:

```text
Memoria Nome, Pontos
Nome = EntraTexto, "Como você se chama?"
Pontos = 2,5
Pontos += 1
Posicione 2, 4; Escreva "Olá, ", Nome, "!"
Se Pontos = "3,5"
    Posicione 4, 4; Escreva "Pontos: ", Pontos
FimSe
FimPrograma
```

## Standalone command-line runner

### Install a release build

When binary packages are available, download the archive for your platform from [GitHub Releases](https://github.com/diegopn/potoo-logic-basic-interpreter/releases), extract it, and place the `potoo-logic-basic` executable somewhere on your `PATH`.

### Build and install from source

Install [Rust and Cargo](https://www.rust-lang.org/tools/install) first. This project requires Rust 1.93 or newer.

```sh
git clone --branch develop https://github.com/diegopn/potoo-logic-basic-interpreter.git
cd potoo-logic-basic-interpreter
cargo install --path . --locked --bin potoo-logic-basic
```

Cargo installs the executable in its configured binary directory, usually `~/.cargo/bin`. Make sure that directory is on your `PATH`.

To build without installing:

```sh
cargo build --release --locked --bin potoo-logic-basic
```

The executable is written to `target/release/` (`potoo-logic-basic.exe` on Windows).

## Use the command-line runner

Run a source file by passing its path:

```sh
potoo-logic-basic saudacao.lbc
```

The runner reads UTF-8 source files, sends text prompts to standard error, reads `EntraTexto` and `Aguarde` key input from standard input, and prints the final text window to standard output. `Mensagem` is written to standard error. File and execution errors include the source line when available.

```sh
potoo-logic-basic --help
```

`CarregaPrograma` resolves relative paths from the main source file's directory and adds the `.lbc` extension when a path has no extension. The CLI provides console input, messages, key waits, timed waits, random numbers, and program loading. Interactive windows, menus, buttons, graphics, multimedia, printing, persistent memories, and database storage require an embedding host; those operations report an error when unavailable in the CLI.

## Use as a Rust library

Add the interpreter as a path dependency when working from a local checkout:

```toml
[dependencies]
potoo-logic-basic-interpreter = { path = "../potoo-logic-basic-interpreter" }
```

`run` is a small entry point for programs that only need text input and the default text grid:

```rust
use std::io::{self, Write};
use potoo_logic_basic_interpreter::run;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
Memoria Nome
Nome = EntraTexto, "Como você se chama?"
Escreva "Olá, ", Nome
FimPrograma
"#;

    let window = run(source, |prompt, _window| {
        eprint!("{prompt} ");
        io::stderr().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.ends_with('\n') {
            input.pop();
            if input.ends_with('\r') {
                input.pop();
            }
        }
        Ok(input)
    })?;

    if let Some(window) = window {
        print!("{window}");
    }
    Ok(())
}
```

For IDE integration, implement the public `Host` trait and call `run_with_host(source, &mut host)`. Host requests are typed operations carrying ordered arguments, block rows, and the current `Window`. Implement only the capabilities available in the embedding application. The trait also provides callbacks for text-grid changes and execution checkpoints, plus methods for memory snapshots, loaded programs, and logical database records. Unsupported capabilities return source-located execution errors. `FimJanela` returns `None`; normal completion and `FimPrograma` return the final window.

Visual, media, printing, memory snapshot, and database storage operations are sent to the embedding host as typed requests. The CLI focuses on text input and output; physical `.LBA` database file compatibility is not claimed.

## Development

Format, check, and test the project with:

```sh
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
```

## Support

If this project is useful to you, you can [sponsor its maintainer](https://github.com/sponsors/diegopn).

## License and project policies

The available author manual is marked all rights reserved. The original executable, source code, and separate license terms could not be verified. This independent Rust project is licensed under GPL-3.0-or-later.

- [Code of conduct](CODE_OF_CONDUCT.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [GPL-3.0-or-later license](LICENSE)
