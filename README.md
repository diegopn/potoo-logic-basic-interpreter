<h1 align="center">Logic Basic Interpreter</h1>

<p align="center">A Rust library and command-line runner for the documented Logic Basic 7.0 language.</p>

<p align="center">
  <a href="https://diegopn.github.io/potoo-site/">Website</a> ·
  <a href="https://diegopn.github.io/potoo-site/manual/logic-basic/">Potoo IDE manual</a>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white"></a>
  <a href="https://doc.rust-lang.org/cargo/"><img alt="Cargo" src="https://img.shields.io/badge/Cargo-DEA584?style=for-the-badge&logo=rust&logoColor=white"></a>
  <img alt="Rust library" src="https://img.shields.io/badge/Rust_library-4A86CF?style=for-the-badge">
  <img alt="CLI" src="https://img.shields.io/badge/CLI-6A4C93?style=for-the-badge">
</p>

Logic Basic is a programming language for learning programming. Its documented commands cover text output, control flow, reusable routines, arrays, windows, visual resources, components, printing, and data files. This project implements the documented Logic Basic 7.0 command language in Rust. The Potoo IDE supplies the visual interface through the interpreter's host API.

## Run a program

Save this program as a UTF-8 file named `saudacao.lbc`:

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

```sh
cargo run --locked --bin potoo-logic-basic -- saudacao.lbc
```

Library callers can use `run` with a callback for `EntraTexto`, or `run_with_host` to provide the language's host capabilities. A host receives typed operations for interactive controls, files, graphics, multimedia, printing, and database storage. `Host::checkpoint` runs before each source statement so an embedding environment can handle pause, single-step, and cancellation. If it returns an interrupted I/O error, the resulting `Error::was_stopped()` is true.

The CLI host supports line-based text input, messages, line-based key waits, timed waits, random numbers, and loading another `.lbc` file relative to the main file. It prints the final text grid to standard output. Interactive GUI controls and the other host capabilities need an embedding application.

## Language support

- `Memoria` declarations support scalar values and one-dimensional arrays, with a total limit of 1,000,000 array elements across the running program. Array indices start at zero. Values can be text, numbers, logical values, colors, objects, sounds, or arrays. `ApagaMemoria` removes a declared memory; `ReorganizaMemoria` has no visible effect because named memory storage has no exposed positional slots.
- Assignments use `=`; arithmetic supports `+ - * / %` and updates support `++ -- += -=`. Text literals use double quotes, decimal literals use a comma in assignments, and `SIM` / `NAO` represent logical values.
- Apostrophes begin comments. Semicolons separate commands on one physical line. Identifiers and commands are matched without regard to ASCII letter case.
- `Se`, `CasoContrario`, `FimSe`, `VaPara`, `FimPrograma`, and `FimJanela` implement conditions, labels, and program termination. A condition may use repeated `E` or repeated `OU`, but the manual prohibits mixing both operators on one `Se` line.
- `Execute Nome()` and `Retorne` implement named routines, including recursive calls. `CarregaPrograma` loads and runs another source file through `Host::load_program`; the CLI adds `.lbc` when the path has no extension.
- `Posicione`, `Escreva`, `LimpaJanela`, `EntraTexto`, `Mensagem`, and `Aguarde` implement text-window output, input, messages, and waits. `Aguarde` supports key input, seconds, object clicks, and button clicks through the host.
- `Unir`, `Separe`, `Alinha`, `Formata`, `RGB`, `Aleatorio`, `Data`, `Hora`, `TestaMouse`, and `ProcureMatriz` are implemented as documented functions or host requests.
- `GravaMemoria` and `CarregaMemoria` use host-provided snapshots. Their physical file format belongs to the host. A missing saved-memory file may return `None`, leaving program memories unchanged.
- `Menu ... FimMenu`, `Botao ... FimBotao`, `Botao Remove`, `Componente`, `Box*`, `Janela`, `Fundo`, `Fonte`, `Cria Objeto/Som ... FimCria`, drawing, transparency, video, sound, and `Imp` commands are sent to the host as typed operations. The interpreter handles the default text grid and follows `Janela Resolucao` changes; one grid is limited to 1,000,000 cells.
- `BD ... FimBD` defines database fields. The interpreter handles records, field values, ordering, searches, deletion, record counts, and compaction. The host owns opening, saving, closing, path resolution, and the physical database format.

The original `.LBA` database format and the original executable were not available for compatibility checks. Database hosts exchange logical records through the public `Host` API; this project does not claim byte-level compatibility with `.LBA` files. The original window's text wrapping, clipping, and cursor details beyond documented examples also could not be confirmed against the original runtime.

## Window and host behavior

The default text grid has 27 rows and 79 columns with zero-based coordinates. Each character occupies one cell. `Escreva` joins its arguments without adding spaces, overwrites cells from the current cursor, and advances to the next row. `LimpaJanela` clears the text cells. `Janela` may resize the text grid and is forwarded to the host for the surrounding visual window. Grid and array storage are bounded to keep malformed programs from requesting excessive memory; `Alinha` also limits its requested width to 1,000,000 characters.

Host operations receive the current `Window`, ordered arguments (including named arguments), and block rows for menus, buttons, database schemas, and resource creation. `window_changed` reports grid or cursor changes. Hosts can implement only the capabilities available in their environment; an unsupported operation produces a source-line error.

## References and license

- [Curso Básico de Logic Basic 7.0, Eleusmário Mariano Rabelo](https://pt.scribd.com/document/420235581/Manual-do-Logic-Basic), the primary reference for the documented language.
- [Potoo IDE Logic Basic manual](https://diegopn.github.io/potoo-site/manual/logic-basic/), supplemental reference.

The available author manual is marked all rights reserved. The original executable, source code, and separate license terms could not be verified. This Rust project is licensed under GPL-3.0-or-later.

## Development

```sh
cargo fmt --all -- --check
cargo check --locked
```

## Project policies

- [Code of conduct](CODE_OF_CONDUCT.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [GPL-3.0-or-later license](LICENSE)
