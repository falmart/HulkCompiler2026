# HULK Compiler

Compilador para el lenguaje **HULK** implementado en Rust. El pipeline completo incluye análisis léxico, sintáctico, semántico e interpretación mediante recorrido del AST.

## Requisitos

- Rust 1.70+ (`rustup` o [rust-lang.org](https://www.rust-lang.org))

## Compilar

```bash
make build
```

Produce el binario `./hulk` en la raíz del proyecto.

## Uso

```bash
# Compilar un archivo fuente
./hulk programa.hulk

# Si no hay errores, se genera ./output (ejecutable)
./output
```

## Códigos de salida

| Código | Significado |
|--------|-------------|
| `0` | Éxito — se generó `./output` |
| `1` | Error léxico |
| `2` | Error sintáctico |
| `3` | Error semántico |

Los errores se reportan en stderr con el formato:
```
(línea,columna) TIPO: mensaje
```

## Características implementadas

- Expresiones, variables (`let`), condicionales (`if/elif/else`), bucles (`while`, `for`)
- Funciones con anotaciones de tipo opcionales
- OOP: clases, herencia (`is`/`inherits`), `self`, `base()`, `is`, `as`
- Protocolos con tipado estructural
- Vectores: literales `[a, b, c]`, comprensiones `[expr | x in iter]`, `new T[n]`
- Funciones de primera clase: lambdas `(x) => expr`, closures
- Macros: `def nombre(@byref, *byname, $varname, valor) { cuerpo }`
- Funciones integradas: `print`, `sin`, `cos`, `tan`, `sqrt`, `exp`, `log`, `rand`, `range`
- Constantes: `PI`, `E`

## Estructura del proyecto

```
Cargo.toml              ← workspace raíz
Makefile
REPORT.md               ← reporte técnico del proyecto
crates/
  hulk_lexer/           ← tokenizador
  hulk_ast/             ← definiciones del AST
  hulk_parser/          ← parser de descenso recursivo
  hulk_semantic/        ← verificador semántico (2 pasadas)
  hulk_interpreter/     ← intérprete de árbol de sintaxis
hulkc/
  src/main.rs           ← CLI
examples/               ← programas de ejemplo
```

## Tests

```bash
cargo test
```

297 pruebas unitarias distribuidas entre los crates de lexer, parser, semántico e intérprete.
