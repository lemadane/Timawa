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

## Why Software Engineering & Performance are Superior in Talas

Not only does Talas function nicely without `static` and `singleton` keywords—it actually produces cleaner, more testable, and significantly faster software because of their absence.

### 1. Why You Don't Miss the `static` Keyword

In languages like Java or C#, `static` is used for 3 primary reasons. Talas solves all 3 far more elegantly:

| Traditional Use of `static` | How Talas Solves It Without `static` | Why Talas Solution is Better |
| :--- | :--- | :--- |
| **1. Utility Functions** (`Math.abs()`) | **Instance Utilities / Primitive Methods**<br>`math = Math(); math.abs(-42)` or `-42.abs()` | Eliminates procedural code inside OOP; clean object scoping. |
| **2. Entry Point** (`public static void main`) | **Application Instance Method**<br>`class Application { main() { ... } }` | Pure OOP consistency; compiler handles native C-ABI bootstrapping. |
| **3. Global State** (`static int counter`) | **Application-Scoped Instance**<br>`final appState = AppState()` | Prevents multi-threaded data races on global variables. |

### 2. Why You Don't Miss a `singleton` Keyword

In Java or C#, static singletons create major software engineering problems:
- **Hidden Dependencies**: Classes secretly reach out to `Database.getInstance()`, making it impossible to see what a class depends on.
- **Testing Nightmares**: Tests share global static state, leading to flaky tests where one test mutates state used by another.
- **Race Conditions**: Multi-threaded access to static singletons requires complex `synchronized` locks.

#### In Talas (Application-Scoped Dependency Injection):
In Talas, a "Singleton" is simply an instance created **once** in `Application.main()` and explicitly passed to classes that need it:

```java
class Application {
    main() {
        // Created ONCE at startup — acts as an application-level singleton
        final db = DatabaseConnection(`postgres://localhost:5432`)

        // Explicitly passed to services
        final userService = UserService(db)
        final orderService = OrderService(db)

        userService.findUser(1000)
    }
}
```

**Why this is superior**:
- Every dependency is 100% visible in the primary constructor `UserService(DatabaseConnection db)`.
- Unit testing is effortless: you can pass a mock `TestDatabaseConnection` without resetting global static state.

### 3. The Secret Performance Boost

By eliminating `static` methods and `extends` (inheritance):
- **Direct Machine Calls**: LLVM compiles every Talas instance method directly to a fixed machine symbol (`@User_birthday`). There are no virtual method table (vtable) pointer lookups.
- **0-Cost Function Calls**: LLVM aggressively inlines method bodies directly into call sites, reducing function call time to 0 nanoseconds.
- **No Thread Lock Bottlenecks**: There are no static class-loader locks or static mutex bottlenecks during startup.

### Summary: The Power of Minimal Keywords

By omitting `static`, `singleton`, `void`, `extends`, `implements`, `public`, `private`, `protected`, `struct`, `union`, and `enum`, Talas reduces language complexity to a tiny, elegant core:

- **1 Type Keyword**: `class`
- **1 Mutability Keyword**: `var` (immutable by default)
- **1 Loop Keyword**: `for`
- **1 Output Directive**: `print` (always includes newline)
- **Visibility by Naming**: `name` (public) vs. `_name` (package-private)

The result is a language that is easy to learn in an afternoon, virtually impossible to introduce thread-race bugs in, and compiles to ultra-fast native executables.

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

## Architecture & Project Structure

For detailed compiler pipeline design, module layout, object layout headers, memory management, and compiler architecture, see:
- [Compiler & Workspace Architecture (`ARCHITECTURE.md`)](ARCHITECTURE.md#2-project--workspace-directory-structure)

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
