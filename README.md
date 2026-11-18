<h1 align="center">Logic Basic Interpreter</h1>

<p align="center">An independent Rust library and command-line runner for the documented Logic Basic 7.0 programming language.</p>

<p align="center">
  <a href="https://diegopn.github.io/potoo-site/">Potoo website</a> ·
  <a href="https://diegopn.github.io/potoo-site/manual/logic-basic/">Potoo IDE Logic Basic manual</a> ·
  <a href="https://github.com/diegopn/potoo-logic-basic-interpreter/releases">Releases</a>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white"></a>
  <a href="https://doc.rust-lang.org/cargo/"><img alt="Cargo" src="https://img.shields.io/badge/Cargo-DEA584?style=for-the-badge&logo=rust&logoColor=white"></a>
  <img alt="Rust library" src="https://img.shields.io/badge/Rust_library-4A86CF?style=for-the-badge">
  <img alt="Command-line interface" src="https://img.shields.io/badge/CLI-6A4C93?style=for-the-badge">
</p>

Logic Basic is a programming language, not a pseudolanguage. This project implements its documented 7.0 command language as a Rust library and a standalone command-line runner. The library is designed for embedding in Potoo IDE, where a host integration can provide the visual interface. This repository does not include a graphical interface.

## Language references

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

## Install the command-line runner

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

## Supported language features

### Source and values

- Scalar and one-dimensional array declarations use `Memoria`. Arrays use zero-based indices and support indexed assignment and `ProcureMatriz`.
- `ApagaMemoria` removes a declared memory. `ReorganizaMemoria` has no visible effect because named memory storage has no exposed positional slots.
- Values include text, numbers, logical values (`SIM` and `NAO`), colors, objects, sounds, and arrays.
- Assignment uses `=`. Arithmetic supports `+`, `-`, `*`, `/`, and `%`; updates support `++`, `--`, `+=`, and `-=`.
- Apostrophes start comments. Semicolons separate commands on one physical line. Commands and identifiers are matched without regard to ASCII letter case.
- Decimal values in assignments use a comma, such as `2,5`. Conditions may compare a number with its quoted decimal form, such as `Se Total = "2,5"`.

### Control flow and text

- `Se`, `CasoContrario`, and `FimSe` provide conditions. A condition may use repeated `E` or repeated `OU`; the documented syntax does not mix both operators on one `Se` line.
- `VaPara`, labels, `FimPrograma`, and `FimJanela` provide jumps and termination.
- `Execute Nome()` and `Retorne` call named routines, including recursive routines.
- `Posicione`, `Escreva`, `LimpaJanela`, `EntraTexto`, `Mensagem`, and `Aguarde` support text-window output, input, messages, and waits.
- Built-in functions include `Unir`, `Separe`, `Alinha`, `Formata`, `RGB`, `Aleatorio`, `Data`, `Hora`, and `TestaMouse`. `ProcureMatriz` searches an array and returns its zero-based index, or `-1` when there is no match.

### Host-backed capabilities

- `Janela`, `Fundo`, `Fonte`, `Menu`, `Botao`, `Componente`, `Box*`, `Cria Objeto/Som`, drawing, transparency, video, and sound commands are forwarded to the host as typed operations.
- `Imp` commands send print operations to the host.
- `GravaMemoria` and `CarregaMemoria` use host-provided snapshots. The host defines the physical file format.
- `BD ... FimBD` defines database fields. The interpreter implements logical records, field values, ordering, `Procure` searches, deletion, record counts, and compaction. The host owns opening, saving, closing, path resolution, and physical storage.
- `CarregaPrograma` loads and executes another source file through the host.

## Runtime limits and compatibility notes

- Array storage is limited to 1,000,000 elements across one execution. A text grid is limited to 1,000,000 cells. `Alinha` is limited to a width of 1,000,000 characters.
- The default text grid has 27 rows and 79 columns, with zero-based coordinates. Each character occupies one cell. `Escreva` joins its arguments without adding spaces, writes from the cursor, then advances to the next row.
- `Janela Resolucao` can resize the text grid. Hosts receive the updated grid and cursor through `window_changed`.
- The original `.LBA` database format and original executable were not available for compatibility checks. Database hosts exchange logical records through the public `Host` API; this project does not claim byte-level compatibility with `.LBA` files.
- Text wrapping, clipping, and cursor behavior beyond the documented examples could not be confirmed against the original runtime.

## Development and tests

Format, check, and test the project with:

```sh
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
```

Integration tests in `tests/` exercise the public runtime and the actual CLI process, including host requests, text input, arrays, routines, memory snapshots, loaded programs, source diagnostics, and window behavior.

## Support

If this project is useful to you, you can [sponsor its maintainer](https://github.com/sponsors/diegopn).

## License and project policies

The available author manual is marked all rights reserved. The original executable, source code, and separate license terms could not be verified. This independent Rust project is licensed under GPL-3.0-or-later.

- [Code of conduct](CODE_OF_CONDUCT.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [GPL-3.0-or-later license](LICENSE)
