# Logic Basic reference coverage

This crate targets the documented Logic Basic 7.0 command language from the author's introductory manual. It does not claim compatibility with another release, the full Windows visual environment, or undocumented behavior. The original executable was unavailable for differential testing.

| Reference area | Implemented profile | Boundary or validation status |
| --- | --- | --- |
| Memories and expressions | Named memories, scalar values, arithmetic, comparison and logical operators, assignment, and documented conversions | ReorganizaMemoria has no observable effect because this interpreter addresses memories by name; allocator slot behavior is unverified. |
| Control and routines | Conditions, loops, labels/jumps, named routines, recursion, and return to the call site | An explicit return stack is used; the manual does not specify the original recursion ceiling. |
| Matrices and text | Zero-based matrices, indexed reads/writes, string helpers, and the documented text grid | Separe counts Unicode scalar values. The manual does not settle normalization, grapheme clusters, encodings, grid clipping, or all cursor behavior. |
| Input and output | EntraTexto, Escreva, Mensagem, prompts, and the default 27×79 text grid | Windows controls, interactive forms, mouse/menu behavior, sounds, images, video, and printing are host services. |
| Files, programs, and database | Program loading and logical database requests can be supplied through Host; the manual's database commands and field model are represented | Physical .LBA byte format and record encoding are not documented in the available manual and are not claimed. |
| Entre | The manual says database fields may be supplied as arguments to Entre | It does not give a callable signature or argument rules; the function remains explicitly unavailable instead of guessing its semantics. |
| CLI and host boundary | Text input/output, key/timed waits, random values, and source-relative program loading | The CLI has no visual window, multimedia surface, persistent-memory file format, or database storage implementation. |

Existing representative coverage is in tests/runtime.rs and tests/cli.rs. It checks the language and host contract, not parity with the unavailable original executable.

## Sources

- [Author's Curso Básico de Logic Basic 7.0](https://pt.scribd.com/document/420235581/Manual-do-Logic-Basic)
- [Supplemental Logic Basic manual](https://diegopn.github.io/potoo-site/manual/logic-basic/)
