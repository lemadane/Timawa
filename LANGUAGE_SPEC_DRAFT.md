# Talas Language Specification Baseline (Draft v0.1.0)

```text
Name: Talas Programming Language
Compiler Command: talas
Source Extension: .talas
Tagline: Sharp, simple, native.
```

---

## 1. Language Identity & Design Goals

Talas is positioned between Go and Java:
* Native performance and simple single-binary deployment like Go.
* Familiar, readable, brace-delimited syntax like Java.
* Pure object-oriented programming model (everything is an instance method of a class).
* Built-in `print(...)` directive for output (always appends a newline, equivalent to Java `System.out.println` or Python `print()`).
* Implicit void return types: methods without an explicit return type default to void. The `void` keyword does not exist in Talas.
* Immutable by default (fields, parameters, local variables).
* Garbage-collected runtime with zero manual memory management boilerplate.
* Statically typed with structural generic constraints and flow-sensitive type narrowing.
* Direct native compilation through LLVM (no transpilation to Java, C, Go, or Rust).
* Lightweight task-based concurrency (long-term goal).

---

## 2. Built-in I/O Directive (`print`)

Talas provides `print(...)` as the sole built-in output directive. `print` **always appends a newline** upon execution (equivalent to Java's `System.out.println` or Python's `print()`).

```java
class Application {
    main() {
        final name = `Lem`
        print(`Hello, ${name}!`)
    }
}
```

---

## 3. Basic Data Types & Semantics

Talas provides exactly six primitive data types (the `void` keyword does not exist):

| Type Specifier | Native Representation | Description | Default Value |
| :--- | :--- | :--- | :--- |
| `string` | Immutable UTF-16 pointer + length | Textual string | `` (empty string) |
| `int` | Signed 32-bit integer (`i32`) | Standard integer | `0` |
| `decimal` | IEEE 754 64-bit float (`double`) | Double-precision floating point | `0.0` |
| `byte` | Unsigned 8-bit integer (`u8`) | Raw byte data | `0` |
| `long` | Signed 64-bit integer (`i64`) | Wide integer | `0` |
| `bool` | 1-bit boolean (`i1`) | Logical truth value | `false` |
| `char` (N/A) | *No distinct char type* | Represented as 1-character `string` | `` |

---

## 4. Unified Class Model & Implicit Void Methods

Every user-defined type in Talas is declared using the `class` keyword. Methods without an explicit return type are **implicitly void** (no `void` keyword).

```java
class User(
    long id,
    string name,
    var int age = 0
) {
    birthday() {
        this._age = this._age + 1
    }

    int getAge() {
        return this._age
    }
}
```

---

## 5. Explicit `this` Requirement

Inside any instance method of a class, every access to an instance field or instance method **must** be explicitly qualified with `this.`.

```java
class Account(long id, var decimal balance) {
    deposit(decimal amount) {
        this._balance = this._balance + amount
    }
}
```

---

## 6. Access Control (Naming-Based Visibility)

- **Unprefixed Identifier** (`Name`, `methodName`): **Public**.
- **Underscore-Prefixed Identifier** (`_Name`, `_methodName`): **Package-Private / Internal**.

---

## 7. Word-Based Boolean Operators & Truthiness

Talas uses word-based logical operators: `not`, `and`, `nand`, `or`, `nor`, `xor`, `xnor`.

Falsy values: `false`, `null`, `0`, `0.0`, `-0.0`, `` ``. All other values are truthy.
