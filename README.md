<h1 align="center">Logic Basic Interpreter</h1>

<p align="center">An initial Rust library and CLI scaffold for the Logic Basic programming language.</p>

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

Logic Basic is a programming language for learning programming, with commands for text, control flow, visual elements, and data files. It is not a pseudolanguage. This repository will target the syntax documented in the linked reference and will state its exact version and supported features as implementation proceeds.

This repository is only a starting scaffold. It does not yet parse or execute Logic Basic source, and no interpreter features are currently implemented. The Rust implementation will be developed here independently from the original language project.

## Original language references

- [Potoo IDE manual](https://diegopn.github.io/potoo-site/manual/logic-basic/)
- [Logic Basic 7.0 author manual](https://pt.scribd.com/document/420235581/Manual-do-Logic-Basic)

These references define the compatibility work. This repository will list its supported syntax, runtime behavior, and limitations after implementation begins.

## Development

Run these commands from the repository root:

```sh
cargo fmt --all -- --check
cargo check
cargo test
```

## Project policies

- [Code of conduct](CODE_OF_CONDUCT.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [GPL-3.0-or-later license](LICENSE)
