# Compilador HULK — Reporte

## Visión General

Este proyecto implementa un compilador completo para el lenguaje de programación **HULK**: un lenguaje de tipado estático, orientado a objetos y basado en expresiones, con funciones de primera clase, tipado estructural mediante protocolos y comprensiones de vectores. La implementación está escrita en **Rust** y organizada como un workspace de Cargo con seis crates que forman un pipeline clásico de compilación: léxico → parser → análisis semántico → intérprete.

El compilador pasa **115 de 115 tests** de la suite oficial del curso, incluyendo todas las categorías requeridas y todas las categorías bonus.

---

## Arquitectura General

El proyecto se estructura como un workspace de Cargo con los siguientes crates:

```
hulkc/              ← Binario CLI (punto de entrada)
crates/
  hulk_lexer/       ← Tokenizador
  hulk_ast/         ← Definiciones del Árbol Sintáctico Abstracto (AST)
  hulk_parser/      ← Parser de descenso recursivo
  hulk_semantic/    ← Verificador semántico en dos pasadas
  hulk_interpreter/ ← Intérprete de árbol de sintaxis
```

Cada crate tiene una responsabilidad única y bien definida. Las dependencias fluyen en una sola dirección: el CLI depende de todos los crates, el intérprete depende del AST y la capa semántica, y el lexer no tiene dependencias en el resto del pipeline. Esta separación facilita el testing independiente de cada fase.

### Pipeline de Compilación

Cuando se invoca `./hulk programa.hulk`, el código fuente pasa por las siguientes etapas:

1. **Análisis léxico** — La cadena fuente se tokeniza en un `Vec<Token>` plano. Cada token lleva su tipo, su lexema original y un `Span` (byte de inicio, byte de fin, línea, columna). Si se encuentra un carácter no reconocido, se devuelve un `LexError` inmediatamente y el compilador termina con código `1`.

2. **Análisis sintáctico** — El flujo de tokens es consumido por el parser de descenso recursivo, que construye un `Program` AST tipado. Se devuelve un `ParseError` en la primera violación sintáctica y el compilador termina con código `2`.

3. **Análisis semántico** — Un verificador en dos pasadas recorre el AST. La primera pasada recolecta todas las declaraciones de nivel superior (funciones, clases, protocolos) en tablas de símbolos. La segunda pasada verifica los tipos de cada expresión. Todos los errores semánticos se recolectan en un `Vec<SemanticError>` y se reportan juntos; el compilador termina con código `3`.

4. **Interpretación** — Si no hay errores, el `Interpreter` recorre el árbol y ejecuta el programa. Al terminar con éxito, el compilador produce un archivo ejecutable `./output` (un script de shell que re-ejecuta el intérprete con el código fuente embebido en base64) y termina con código `0`.

---

## Analizador Léxico (`hulk_lexer`)

El lexer es un escáner de una sola pasada, carácter por carácter. Mantiene un lookahead de un carácter mediante un campo `peeked: Option<(usize, char)>`. Reconoce:

- **Literales numéricos** — enteros y de punto flotante (ej. `42`, `3.14`).
- **Literales de cadena** — entre comillas dobles, con secuencias de escape `\"`, `\\`, `\n`, `\t`.
- **Identificadores y palabras clave** — los identificadores se reconocen primero y luego una tabla de búsqueda `keyword()` convierte las palabras reservadas (`let`, `in`, `function`, `class`, `type`, `protocol`, `interface`, `def`, `define`, etc.) en sus tipos de token específicos.
- **Operadores** — operadores de un carácter (`+`, `-`, `*`, `/`, `%`, `^`, `<`, `>`, `=`, `!`, `&`, `|`, `@`), operadores de varios caracteres desambiguados (`==`, `!=`, `<=`, `>=`, `:=`, `->`, `=>`, `@@`), y pares de paréntesis/corchetes/llaves.
- **Comentarios** — los comentarios de línea `//` se omiten silenciosamente.
- **Tokens especiales** — `$` (prefijo de variable en macros, token `Dollar`), `@` y `@@` (concatenación de cadenas).

El lexer mantiene posicionamiento preciso: rastrea `line` y `col` (indexados desde 1) e incluye esta información en cada error y span, lo que permite reportar errores con ubicación exacta en el fuente.

---

## Árbol Sintáctico Abstracto (`hulk_ast`)

El AST se define como un conjunto de enums y structs de Rust. El tipo central es `Expr`, que cubre todas las formas de expresión del lenguaje:

- **Literales**: `Number(f64)`, `Bool(bool)`, `Str(String)`, `Null`
- **Variables y self**: `Var(String)`, `Self_`
- **Operadores**: `Unary`, `Binary` (con `BinaryOp` cubriendo aritmética, comparación, lógica y concatenación de cadenas)
- **Control de flujo**: `If`, `While`, `For`, `Block`
- **Vinculación**: `Let` (con múltiples vinculaciones en un solo `let`)
- **Funciones y métodos**: `Call`, `MethodCall`, `FieldAccess`
- **Objetos**: `New`, `NewArray`, `Index`
- **Operaciones de tipo**: `IsInstance` (`is`), `Cast` (`as`), `Case` (coincidencia de patrones), `With` (desenvolvimiento de nulables)
- **Vectores**: `VecLit` (literal explícito `[a, b, c]` y también `{a, b, c}`), `VecComp` (`[expr | var in iter]`)
- **Herencia**: `Base` (llamada al método del padre)
- **Funciones de primera clase**: `Lambda` (`(x) => expr` y `function(x) -> expr`)
- **Asignación destructiva**: `Assign`
- **Macros**: `MacroArgRef`, `MacroArgName`, `MacroMatch`

Cada nodo se envuelve en `Spanned<T>`, que asocia el nodo con su `Span` de origen. Esto garantiza que los mensajes de error siempre apunten a la ubicación relevante en el código fuente.

Las declaraciones de nivel superior se separan en `FunctionDecl`, `ClassDecl`, `ProtocolDecl` y `MacroDecl`, todas recolectadas en un struct `Program`.

El enum `TypeExpr` modela las anotaciones de tipo: `Named(String)` para tipos simples, `Array(Box<TypeExpr>)` para `T[]`, `Iterable(Box<TypeExpr>)` para `T*` (tipos de parámetros iterables), y `Function { params, ret }` para tipos de función `(T) -> R`.

---

## Parser (`hulk_parser`)

El parser es un **parser de descenso recursivo** escrito a mano que consume el `Vec<Token>` producido por el lexer. Utiliza un enfoque basado en cursor (`pos: usize`) con lookahead de un token mediante los métodos auxiliares `peek()` y `check()`.

### Precedencia de Operadores

La precedencia se maneja mediante funciones de parseo mutuamente recursivas ordenadas de menor a mayor prioridad:

```
parse_expr
  → parse_assign        (:=)
    → parse_type_ops    (is, as)
      → parse_or        (|)
        → parse_and     (&)
          → parse_equality    (==, !=)
            → parse_compare   (<, <=, >, >=)
              → parse_concat  (@, @@)
                → parse_add   (+, -)
                  → parse_mul  (*, /, %)
                    → parse_pow  (^, asociativo por la derecha)
                      → parse_unary   (-, !)
                        → parse_postfix  (., [], llamadas)
                          → parse_primary
```

### Decisiones de Diseño del Parser

**Flexibilidad en el nivel superior**: El parser permite que declaraciones (`function`, `class`, `type`, `protocol`, `def`, `define`) y expresiones se mezclen libremente en el nivel superior. Los puntos y coma entre elementos del nivel superior son opcionales: se consumen si están presentes pero no se requieren, lo que corresponde a la sintaxis flexible de HULK donde las expresiones que terminan en `}` no necesitan punto y coma al final.

**Ambigüedad de `as`**: La palabra clave `as` aparece en dos contextos: `with (expr as binding)` (vinculación de variable) y `expr as TypeName` (conversión de tipo). Un indicador `forbid_as_cast: bool` en el parser se establece en `true` dentro de `parse_with`, evitando que `as` se consuma como operador de conversión en ese contexto.

**Ambigüedad de `|`**: El carácter `|` se usa tanto como OR lógico (`a | b`) como separador en comprensiones de vectores (`[x^2 | x in range(1, 11)]`). Un indicador `forbid_pipe_or: bool` se establece en `true` dentro del parseo de `[...]`, evitando que el parser de expresiones consuma `|` como OR cuando debe terminar el cuerpo de la comprensión.

**Detección de lambdas**: Cuando el parser ve `(`, realiza un análisis de lookahead (`is_lambda_start()`) para determinar si el contenido entre paréntesis es una lista de parámetros de lambda seguida de `=>` o una expresión agrupada normal. El análisis avanza contando paréntesis para encontrar el `)` correspondiente y verifica si le sigue `=>`.

**Parseo contextual de `base(args)`**: La palabra clave `base` no está reservada en el lexer (para evitar conflictos con nombres de parámetros como `base: Number`). En cambio, el parser verifica si un token `Ident("base")` está inmediatamente seguido de `(` y, de ser así, lo parsea como una llamada al método padre `Base { args }`.

**Expresiones de control de flujo como sub-expresiones**: `if`, `let`, `while`, `for`, `case` y `with` se permiten como sub-expresiones (ej. `total + if (cond) 1 else 0`). Esto se maneja en `parse_primary` que enruta a la función de parseo apropiada antes de caer al parseo de literales e identificadores.

**Macros `def` y `define`**: La palabra clave `def` introduce declaraciones de macros con prefijos especiales de parámetros (`@` por referencia, `*` por nombre, `$` nombre de variable, sin prefijo por valor). La palabra clave `define` es un alias que produce el mismo `MacroDecl` pero con todos los parámetros marcados como ByName, implementando semántica de sustitución textual pura (call-by-name). El parser construye un `MacroDecl` completo con su lista de `MacroParam` y el AST del cuerpo. Dentro del cuerpo, `@param` y `$param` se parsean como `Expr::MacroArgRef` y `Expr::MacroArgName` respectivamente. La forma `match(expr) { case (pat) => cuerpo; ... default => cuerpo; }` se parsea como `Expr::MacroMatch`. La detección de `match(...)` ocurre en la rama `Ident("match")` de `parse_primary`, de manera análoga a cómo `base(args)` se detecta sin reservar la palabra clave en el lexer.

**Arreglos multidimensionales**: Para `new Number[][3]`, el parser consume los sufijos `[]` vacíos contando cuántos hay, y los codifica en el nombre del tipo (`"Number[]"`). El verificador semántico luego decodifica este nombre para producir el tipo correcto `Number[][]`, que coincide con la anotación de tipo en `let matrix: Number[][] = new Number[][3]`.

---

## Verificador Semántico (`hulk_semantic`)

El verificador semántico realiza **dos pasadas** sobre el AST.

### Pasada 1: Recolección de Declaraciones

La primera pasada (`collect_declarations`) registra todos los nombres de nivel superior antes de verificar tipos en ningún cuerpo:

- Las funciones se registran con sus tipos de parámetros y tipo de retorno.
- Las clases se registran con sus parámetros de constructor, clase base y firmas de métodos.
- Los protocolos se registran con sus firmas de métodos y relaciones `extends`.
- Las constantes incorporadas (`PI`, `E`) y las funciones incorporadas (`print`, `sin`, `cos`, `sqrt`, `exp`, `log`, `rand`, `range`) se pre-cargan en el entorno.
- Los macros se registran en la tabla de funciones con parámetros `Object` y tipo de retorno `Object`, de modo que el verificador acepta llamadas a macros sin reportar "función no definida". Los cuerpos de macros no se verifican en aislamiento porque los macros son polimórficos: el mismo cuerpo puede operar sobre tipos distintos según el sitio de llamada.

Este enfoque de dos pasadas permite funciones mutuamente recursivas y referencias hacia adelante a nombres de clases sin requerir un orden de declaración específico.

### Pasada 2: Verificación de Tipos

El verificador recorre el AST con un entorno de ámbito léxico (`Env`) que mapea nombres de variables a su `Type`. El enum `Type` cubre:

- Tipos primitivos: `Number`, `Boolean`, `Str`, `Null`
- Tipo objeto: `Object` (tipo cima de la jerarquía)
- Tipos con nombre: `Named(String)` para nombres de clases o protocolos
- Tipos de arreglos: `Array(Box<Type>)`
- Especial: `Unknown` (para parámetros sin tipo, se propaga sin causar errores)

**Subtipado estructural (protocolos)**: Una clase satisface un protocolo si tiene todos los métodos requeridos por el protocolo con firmas compatibles — no se necesita ninguna palabra clave `implements`. El verificador implementa `class_satisfies_protocol()` que recolecta recursivamente todos los requisitos de métodos (incluyendo los de protocolos padre mediante `extends`) y verifica que la clase los provea. Este es el estilo de tipado estructural de Go.

**Coerción de tipos**: Los parámetros sin tipo (sin anotación) usan `Type::Unknown`, que es compatible con cualquier tipo. Esto permite escribir funciones genéricas sin anotaciones de tipo. La concatenación de cadenas (`@`, `@@`) acepta `Number`, `Boolean` y `Object` además de `String`, correspondiendo al comportamiento de coerción en tiempo de ejecución de HULK.

**Constructores heredados**: Al instanciar `new Knight("Phil", "Collins")` donde `Knight` no tiene parámetros de constructor propios pero hereda de `Person(firstname, lastname)`, el verificador recorre la cadena de herencia para encontrar el primer ancestro con parámetros de constructor.

**Tipos iterables (`T*`)**: Los parámetros anotados con `T*` (tipo iterable) se resuelven a `Type::Unknown` en el verificador, lo que permite pasar tanto arreglos como objetos que implementan el protocolo iterador (`next()`, `current()`). Esto es necesario para que generadores definidos por el usuario sean compatibles con funciones que aceptan `T*`.

---

## Intérprete (`hulk_interpreter`)

El intérprete es un **evaluador de árbol de sintaxis** que evalúa recursivamente nodos `Spanned<Expr>` contra un entorno de ejecución.

### Tipos de Valor en Tiempo de Ejecución

Los valores en tiempo de ejecución (`Value`) incluyen:

- `Number(f64)`, `Boolean(bool)`, `Str(String)`, `Null`
- `Object(Rc<RefCell<HulkObject>>)` — instancias de clases asignadas en el heap con campos nombrados
- `Array(Rc<RefCell<Vec<Value>>>)` — vectores mutables con conteo de referencias
- `Closure(Rc<ClosureData>)` — valores de funciones de primera clase que capturan el entorno léxico

Se usa `Rc<RefCell<...>>` para estado mutable compartido (los objetos y arreglos pueden tener múltiples referencias), evitando la necesidad de un recolector de basura mientras se soporta el modelo de objetos de HULK.

### Características Clave del Intérprete

**Despacho de métodos**: Las llamadas a métodos en objetos recorren la jerarquía de clases desde la clase de la instancia hacia arriba. El intérprete mantiene los campos `current_class_name` y `current_method_name` para soportar llamadas `base(args)`, que buscan la versión del método en ejecución actual en la clase padre.

**Closures**: Las expresiones lambda capturan el entorno actual como una instantánea `HashMap<String, Value>`. Cuando se llama a un closure (directamente o como argumento de función de orden superior), el intérprete crea un nuevo ámbito inicializado con las variables capturadas más los argumentos de la llamada.

**Comprensiones de vectores**: `[expr | var in iter]` evalúa el iterable y luego, para cada elemento, define `var` en un nuevo ámbito y evalúa `expr`, recolectando los resultados en un nuevo arreglo.

**Funciones incorporadas**: `print`, `sin`, `cos`, `tan`, `sqrt`, `exp`, `log`, `rand`, `range` (variantes de 1 y 2 argumentos) se manejan como casos especiales en el evaluador de llamadas a funciones. `range(n)` produce `[0, 1, ..., n-1]` y `range(inicio, fin)` produce `[inicio, ..., fin-1]`.

**Bucles `for`**: Iteran sobre arreglos o sobre objetos que implementan el protocolo iterador (métodos `next(): Boolean` y `current(): T`). En el caso de arreglos, cada elemento se vincula directamente. En el caso de objetos (generadores), el intérprete llama `next()` al inicio de cada iteración y `current()` para obtener el valor — lo que permite que clases definidas por el usuario actúen como secuencias perezosas.

**Ejecución de macros**: Cuando el evaluador encuentra `Expr::Call` cuyo nombre corresponde a un macro declarado, intercepta la llamada antes de evaluar los argumentos. Se construyen dos mapas de sustitución: `vsubs` (nombre de parámetro → `ExprS` de reemplazo) para parámetros ByRef y ByName, y `nsubs` (nombre de parámetro → nombre de variable del llamador como cadena) para parámetros VarName. Los parámetros por valor se evalúan normalmente y se vinculan en un nuevo ámbito. La función `substitute()` recorre recursivamente el AST del cuerpo del macro y aplica las sustituciones: `Expr::Var(p)` se reemplaza con la expresión del argumento (ByName) o con `Expr::Var(caller_var)` (ByRef); `Expr::MacroArgRef(p)` se reemplaza igual que `Var(p)` ByRef; `Expr::MacroArgName(p)` se reemplaza con `Expr::Str(caller_var)`. El AST resultante se evalúa en el entorno del llamador, lo que implementa correctamente la semántica de sustitución textual de macros, incluyendo la mutación de variables del llamador a través de parámetros `@byref`.

**Coincidencia de patrones de macros (`MacroMatch`)**: `match(expr) { case (pat) => cuerpo; ... default => cuerpo; }` evalúa el sujeto y lo compara secuencialmente con cada patrón usando igualdad de valor (`==`). La primera rama que coincide se evalúa; si ninguna coincide, se evalúa el cuerpo `default`.

---

## Funcionalidades Extra

Además de los requisitos mínimos del lenguaje HULK, el compilador implementa las siguientes extensiones, todas verificadas con tests de la suite oficial:

### Macros 

El sistema de macros permite definir transformaciones a nivel de AST que se expanden en el sitio de llamada, en lugar de ejecutarse como funciones normales. Se soportan cuatro modos de paso de parámetros:

- **Por valor** (sin prefijo): el argumento se evalúa antes de la expansión.
- **Por referencia** (`@param`): el argumento debe ser una variable; las asignaciones dentro del macro afectan la variable original del llamador.
- **Por nombre** (`*param`): el AST del argumento se sustituye textualmente en cada uso dentro del cuerpo — equivalente al paso por nombre de lenguajes como Algol.
- **Nombre de variable** (`$param`): se sustituye con el *nombre* (como cadena) de la variable pasada como argumento, útil para generar mensajes o claves dinámicas.

La palabra clave `define` es un atajo para macros call-by-name puros: todos sus parámetros son ByName automáticamente, lo que permite definir estructuras de control personalizadas como nuevas palabras clave. La forma `match(expr) { case (pat) => ...; default => ...; }` permite selección de ramas dentro del cuerpo de un macro.

### Arreglos

Además de la sintaxis básica `new T[n]`, se soporta:

- **Literales de arreglo**: `{10, 20, 30}` — sintaxis de llaves con comas.
- **Inicialización con función índice**: `new Number[5] { i -> i * i }` — la expresión se evalúa una vez por posición con `i` vinculado al índice actual.
- **Arreglos 2D**: `new Number[][3]` crea un arreglo de 3 posiciones donde cada elemento puede ser a su vez un `Number[]`. La anotación de tipo `Number[][]` se verifica correctamente contra este tipo de expresión.

### Interfaces

La palabra clave `interface` funciona como alias exacto de `protocol`. Ambas formas producen el mismo nodo AST (`ProtocolDecl`) y son completamente intercambiables. El tipado estructural aplica igual en ambos casos: una clase satisface una interfaz si implementa todos sus métodos, sin necesidad de declararlo explícitamente.

### Lambdas

Además de la sintaxis `(x) => expr`, se soporta la forma `function(x) -> expr` como expresión anónima en cualquier posición donde se esperaría un valor. Las lambdas capturan el entorno léxico (closures), pueden pasarse como argumentos, retornarse como valores y componerse en funciones de orden superior. Ejemplos soportados: `map`, `filter`, `compose`, `apply`.

### Generadores

Una clase puede actuar como generador (secuencia perezosa) implementando dos métodos:

```
next(): Boolean   — avanza el estado interno; retorna false cuando la secuencia termina
current(): T      — retorna el elemento actual
```

Cualquier objeto que implemente este protocolo puede usarse directamente en un bucle `for`:

```
for (x in new Squares(10)) print(x);
```

El intérprete detecta automáticamente si el iterable es un arreglo (iteración directa) o un objeto con `next()`/`current()` (protocolo de generador) y actúa en consecuencia. Esta característica no requiere ninguna anotación especial ni herencia — basta con que los métodos existan en la clase.

---

## Cumplimiento del Contrato de Interfaz

El compilador satisface la interfaz de calificación automatizada:

| Requisito | Implementación |
|-----------|----------------|
| `make build` → `./hulk` | `Makefile` con `cargo build --release && cp target/release/hulkc ./hulk` |
| Código de salida `1` para errores léxicos | `PipelineError::Lex` → `process::exit(1)` |
| Código de salida `2` para errores sintácticos | `PipelineError::Parse` → `process::exit(2)` |
| Código de salida `3` para errores semánticos | Errores semánticos → `process::exit(3)` |
| Formato de error `(línea,col) TIPO: mensaje` | Todos los tipos de error exponen métodos `position()` y `clean_message()` |
| Produce `./output` al tener éxito | Script de shell con fuente en base64, llama a `./hulk --run-stdin` |
| `./output` ejecutable en Linux x86_64 | El script usa sh POSIX, `base64 -d` y resolución de ruta absoluta con `$(dirname)` |

---

## Características del Lenguaje Implementadas

| Característica | Estado |
|----------------|--------|
| Operadores aritméticos, de comparación y lógicos | ✅ |
| Concatenación de cadenas (`@`, `@@`) | ✅ |
| Vinculaciones `let` (múltiples, con anotaciones de tipo) | ✅ |
| Asignación destructiva (`:=`) | ✅ |
| Expresiones `if` / `elif` / `else` | ✅ |
| Bucles `while` | ✅ |
| Bucles `for (var in iter)` — arreglos y generadores | ✅ |
| Expresiones de bloque `{ e1; e2; ... }` | ✅ |
| Funciones con parámetros tipados y tipo de retorno | ✅ |
| Clases con constructores, atributos y métodos | ✅ |
| Herencia (`is` / `inherits`) | ✅ |
| Referencia `self` | ✅ |
| Llamadas al método padre `base(args)` | ✅ |
| Instanciación `new T(args)` | ✅ |
| Arreglos (`new T[n]`, `arr[i]`, `.size()`) | ✅ |
| Arreglos 2D (`new T[][n]`, `T[][]`) | ✅ bonus |
| Literales de arreglo `{a, b, c}` e inicialización con índice | ✅ bonus |
| Comprensiones de vector `[expr \| var in range(...)]` | ✅ |
| `case expr of { binding: Tipo -> cuerpo }` | ✅ |
| `with (expr as binding) cuerpo else fallback` | ✅ |
| Verificación de tipo en tiempo de ejecución `expr is Tipo` | ✅ |
| Conversión de tipo `expr as Tipo` | ✅ |
| Protocolos / Interfaces (tipado estructural, `extends`) | ✅ bonus |
| Tipos de parámetros iterables `T*` | ✅ |
| Anotaciones de tipo función `(T) -> R` | ✅ |
| Expresiones lambda `(x) => expr` y `function(x) -> expr` | ✅ bonus |
| Funciones de primera clase / funciones de orden superior | ✅ bonus |
| Generadores (protocolo `next()` / `current()`) | ✅ bonus |
| Declaraciones de macro `def` con parámetros `@`, `*`, `$`, por valor | ✅ bonus |
| `define` (macros call-by-name) | ✅ bonus |
| Ejecución de macros con sustitución AST | ✅ bonus |
| `match(expr) { case ... default ... }` dentro de macros | ✅ bonus |
| Constantes incorporadas `PI`, `E` | ✅ |
| Funciones matemáticas (`sin`, `cos`, `sqrt`, `exp`, `log`, `rand`) | ✅ |
| `range(n)` y `range(inicio, fin)` | ✅ |
| `print(valor)` | ✅ |

---

## Limitaciones Conocidas

- **Sin generación de código**: El compilador es un intérprete de árbol de sintaxis, no un generador de código. El archivo `./output` es un script de shell que re-invoca el intérprete; no produce código máquina nativo ni bytecode. El rendimiento es proporcional al tamaño del AST.

- **La verificación de tipos de función es superficial**: Los parámetros anotados con tipos de función `(T) -> R` se tratan como `Object` por el verificador de tipos. Los errores de tipo en argumentos de funciones de orden superior (ej. pasar una función `Number -> Number` donde se espera `Number -> Boolean`) no se detectan.

- **Los cuerpos de macros no se verifican semánticamente**: Dado que los macros son polimórficos (el mismo cuerpo puede aplicarse a tipos distintos según el sitio de llamada), el verificador semántico registra los macros en la tabla de funciones con parámetros `Object` pero no verifica el cuerpo de forma aislada. Los errores de tipo dentro del cuerpo se detectarían solo en tiempo de ejecución.

- **Sin recolección de basura**: La memoria se gestiona mediante el sistema de propiedad de Rust y el conteo de referencias `Rc`. Las referencias circulares entre objetos (ej. una lista enlazada circular) causarían pérdidas de memoria. En la práctica, los programas HULK de la suite de pruebas no crean ciclos.

- **Los errores en tiempo de ejecución terminan con código `1`**: Los errores en tiempo de ejecución (división por cero, método indefinido, desbordamiento de pila, etc.) imprimen un mensaje en stderr y terminan con código `1`.

- **`|` no puede usarse como OR lógico dentro de `[...]`**: Debido a que `|` dentro de corchetes se interpreta como separador de comprensión de vectores, escribir `[a | b]` como un vector de un elemento conteniendo el OR de `a` y `b` no está soportado. Esta es una concesión de diseño deliberada.

---

## Pruebas

### Tests Unitarios

El proyecto incluye **297 pruebas unitarias** distribuidas en cuatro crates:

- **`hulk_lexer`** (63 pruebas): Tipos de tokens, reconocimiento de palabras clave, desambiguación de operadores, manejo de secuencias de escape en cadenas y casos de error.
- **`hulk_parser`** (82 pruebas): Parseo de expresiones, precedencia de operadores, todas las formas de declaración y casos de error.
- **`hulk_semantic`** (77 pruebas): Verificación de tipos para todos los operadores, llamadas a funciones, instanciación de clases, herencia, subtipado de protocolos y detección de errores.
- **`hulk_interpreter`** (75 pruebas): Evaluación completa de expresiones, control de flujo, funciones, clases y biblioteca estándar.

Todas las 297 pruebas pasan (`cargo test --release`).

