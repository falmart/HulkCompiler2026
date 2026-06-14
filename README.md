# Hulk Compiler (Rust) — Workspace Skeleton

Este repositorio contiene el esqueleto local para el compilador de HULK implementado en Rust.

- Coloca el PDF de la especificación en `resources/hulk-docs.pdf` o usa el script `scripts/import_resources.sh`.
- Los crates están dentro de `crates/` y se pueden construir con `cargo build -p hulk-cli`.

Estructura inicial:

- `crates/hulk-ast` — definiciones AST
- `crates/hulk-lexer` — lexer (por implementar)
- `crates/hulk-parser` — parser (por implementar)
- `crates/hulk-checker` — análisis semántico y typechecker
- `crates/hulk-vm` — VM y runtime bytecode
- `crates/hulk-cli` — interfaz de línea de comandos

Para importar el PDF desde la ruta de descargas del sistema (local), ejecutar:

```bash
./scripts/import_resources.sh
```
