# Talas Programming Language

> **Sharp, simple, native.**

[![Language Specification](https://img.shields.io/badge/spec-v0.1.0-blue.svg)](LANGUAGE_SPEC_DRAFT.md)
[![Compiler Status](https://img.shields.io/badge/compiler-Rust%20%2B%20LLVM-orange.svg)](ARCHITECTURE.md)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

**Talas** is an independent, pure object-oriented programming language designed for native performance, low memory footprint, and high safety. Positioned between Go and Java, Talas combines native single-binary deployment with familiar brace syntax, pure object-oriented design, implicit void return types, default immutability, built-in `print` directive, and direct native code generation via LLVM.

---

## Language Core Principles & Policies

### 1. Pure Object-Oriented (No `static`, No Free Functions)
- **No Static Methods, Fields, or Classes**: Every executable function in Talas must be an instance method belonging to a class object.
- **No Free-Standing Functions**: All code is encapsulated inside classes.
- **Application Entry Point**: The application entry point is an instance method (`main()`) on an `Application` class. The compiler automatically generates the native C-ABI wrapper.
- **Singletons via Dependency Injection**: Singletons are created once at application startup inside `Application.main()` and passed via primary constructors.

### 2. Zero Inheritance & Maximum Performance
- **No `extends` or Abstract Classes**: Talas has **no inheritance**.
- **0-Byte Vtable Overhead**: Because there is no inheritance, every method call is 100% statically known at compile time. There are no virtual method table (vtable) lookups.
- **Aggressive Inlining**: LLVM inlines instance method bodies directly into caller sites.
- **Cache Locality**: Class instance headers do not store virtual pointer (`vptr`) fields, resulting in smaller memory headers and superior CPU L1/L2/L3 cache hit rates.

### 3. Unified `class` Model (No `struct`, `record`, `union`, or `enum`)
- **Single Type Keyword (`class`)**: `class` is the sole user-defined type keyword.
- **Primary Constructor Fields**: `class User(long id, string name, var int age)` automatically generates package-private backing fields (`_id`, `_name`, `_age`) and public getter methods (`id()`, `name()`, `age()`).
- **Sum Classes for Variants**: Discriminated unions/enums are declared via Sum Classes (`class Status { case Pending; case Approved(long id) }`).

### 4. No `void` Keyword & Implicit Void Return Types
- **No `void` Keyword**: The `void` keyword does not exist in Talas.
- **Implicit Void Return Type**: Methods declared without a return type automatically return void (`main() { ... }` or `birthday() { ... }`).
- **Explicit Return Types**: Methods returning values specify the return type before the method name (`int getAge() { return this._age }`).

### 5. Explicit `this.` Qualification
- All instance field and method accesses inside a class **must** be explicitly prefixed with `this.`. Unqualified access produces a compiler error.

### 6. Naming-Based Access Control
- Visibility is governed by identifier prefixes (`name` = public, `_name` = package-private/internal). There are no `public`, `private`, or `protected` keywords.

### 7. Built-in `print` Directive
- `print(...)` is the sole built-in output directive.
- **Always Appends Newline**: `print` always appends a newline (equivalent to Java's `System.out.println` or Python's `print()`).

### 8. Truthy & Falsy Conditions
- Conditions in `if (condition)` and `for (condition)` accept truthy and falsy values:
  - **Falsy**: `false`, `null`, `0`, `0.0`, `-0.0`, `` `` (empty string).
  - **Truthy**: `true`, non-zero numbers, non-empty strings, class instances, anonymous objects.
- **Flow-Sensitive Narrowing**: Evaluating a nullable `User?` in an `if (user)` block automatically narrows `user` to non-null `User`.

### 9. Word-Based Boolean Operators
- Logical operators use English words: `not`, `and`, `nand`, `or`, `nor`, `xor`, `xnor`. Output is always primitive `bool`.

### 10. First-Class Nullability & Optional Operators
- Native `null` (never `nil`) with postfix `Type?` (`User?`).
- Optional chaining (`?.`), null coalescing (`??`), and null-coalescing assignment (`??=`).

### 11. Collections Policy (`Array<T>`, `List<T>`, `Map<K, V>`)
- **No Interfaces Policy**: Collections are concrete generic classes provided by the standard library.
- **`Array<T>`**: Fixed-size contiguous buffer (`Array<int>(10)`).
- **`List<T>`**: Dynamic growable sequence (`[1, 2, 3]` literal syntax, `.add()`, `.remove()`).
- **`Map<K, V>`**: Key-value hash map (`[1000L: `Lem`]` literal syntax, bracket lookup returns `V?`, key-value `for (k, v in map)` iteration).

### 12. Concurrency (`async` / `await`)
- **Instance Method Spawning**: Spawning a task executes an instance method concurrently: `async worker.process(1)`.
- **$M:N$ Scheduler**: Maps $M$ user-space lightweight tasks onto $N$ native OS threads using growable $2\text{ KB}$ stacks.
- **Inter-Task Channels**: Communication via typed `Channel<T>` objects.

### 13. Lambdas & Method References
- **Brace Syntax**: `{ (x) -> x * 2 }`.
- **Compiler Lowering**: Lowered to synthetic, immutable single-method closure classes.
- **Method References**: `this.methodName` syntax for instance method callbacks.

---

## Code Example

```java
class User(
    long id,
    string name,
    var int age = 0
) {
    // Implicit void return type (no 'void' keyword)
    birthday() {
        this._age = this._age + 1
    }

    // Explicit return type
    int getAge() {
        return this._age
    }
}

class Application {
    // Implicit void entry point
    main() {
        final user = User(1000, `Lem`, 35)
        user.birthday()

        if (user.getAge() > 30) {
            // Built-in print directive (always appends newline like Java println)
            print(`User ${user.name()} is ${user.getAge()} years old.`)
        }

        // Map example
        final userMap = [1000L: user]
        for (id, u in userMap) {
            print(`User ID: ${id}, Name: ${u.name()}`)
        }
    }
}
```

---

## Project Structure

```text
Talas/
├── Cargo.toml                      # Workspace config
├── README.md                       # Main project README
├── LANGUAGE_SPEC_DRAFT.md          # Baseline language specification
├── ARCHITECTURE.md                 # Compiler & runtime architecture
├── DECISIONS_NEEDED.md             # 40 unresolved feature resolutions
├── MILESTONES.md                   # 14 phased milestones & roadmap
├── IMPLEMENTATION_PLAN.md          # Master implementation plan
├── compiler/                       # Crate: talas-compiler
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                 # talas CLI driver
│       ├── lib.rs                  # Compiler library
│       ├── diagnostics/            # Diagnostic reporting & span tracking
│       ├── lexer/                  # Backtick string & token scanner
│       ├── parser/                 # Unified class Pratt parser
│       ├── resolver/               # Scope & symbol resolver
│       ├── types/                  # Type representations
│       ├── semantics/              # Explicit 'this' & immutability checker
│       ├── ir/                     # SSA CFG IR builder
│       └── llvm/                   # Textual LLVM IR emitter & C-main builder
├── runtime/                        # Crate: talas-runtime (Native C-ABI static library)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                  # Runtime init & cleanup
│       ├── memory.rs               # Allocator primitives
│       ├── strings.rs              # UTF-16 string runtime
│       ├── io.rs                   # System I/O wrappers
│       ├── gc.rs                   # GC stubs
│       └── collections.rs          # Runtime collection headers
└── standard-library/
    └── prelude.talas
```

---

## Quick Start (Ubuntu / Linux)

### 1. Prerequisites

Install system build tools and LLVM:

```bash
sudo apt update
sudo apt install -y build-essential clang lld llvm cmake curl git
```

Install Rust via `rustup`:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

### 2. Build & Install

```bash
cargo build --release
sudo cp target/release/talas-compiler /usr/local/bin/talas
```

### 3. Usage Commands

```bash
# Check source code for syntax and semantic errors
talas check app.talas

# Emit textual LLVM IR (.ll)
talas emit-llvm app.talas app.ll

# Compile to native executable
clang app.ll -L target/release -ltalas_runtime -o app
./app
```

---

## Documentation Links

- [Language Specification Baseline (`LANGUAGE_SPEC_DRAFT.md`)](LANGUAGE_SPEC_DRAFT.md)
- [Compiler & Runtime Architecture (`ARCHITECTURE.md`)](ARCHITECTURE.md)
- [Unresolved Design Decisions (`DECISIONS_NEEDED.md`)](DECISIONS_NEEDED.md)
- [Phased Milestones & Roadmap (`MILESTONES.md`)](MILESTONES.md)
- [Master Implementation Plan (`IMPLEMENTATION_PLAN.md`)](IMPLEMENTATION_PLAN.md)

---

## License

Talas is open-source software licensed under the [MIT License](LICENSE).
