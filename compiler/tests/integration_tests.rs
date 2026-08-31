use talas_compiler::{IrBuilder, Lexer, Parser, TextualLlvmEmitter, TypeChecker};

#[test]
fn test_talas_end_to_end_pipeline() {
    let source_code = r#"
        class Status {
            case Pending
            case Approved(long orderId)
            case Rejected(string reason)
        }

        class User(
            long id,
            string name,
            var int age = 0
        ) {
            birthday() {
                this._age = this._age + 1
            }
        }

        class Application {
            main() {
                final user = User(1000, `Lem`, 35)
                user.birthday()

                final obj = { id: 1000, name: `Lem` }
                print(`User birthday updated!`)
            }
        }
    "#;

    // 1. Lexing & Parsing
    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty(), "Parsing failed: {:?}", parser.diagnostics);

    // 2. Type Checking & Semantic Verification
    let mut checker = TypeChecker::new();
    let is_valid = checker.check_program(&program);
    assert!(is_valid, "Type checking failed: {:?}", checker.diagnostics);

    // 3. IR Lowering to SSA CFG
    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);
    assert_eq!(ir_program.classes.len(), 3);

    // 4. LLVM IR Generation
    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(llvm_ir.contains("%User = type {"));
    assert!(llvm_ir.contains("define void @User_birthday"));
    assert!(llvm_ir.contains("call void @talas_print_str"));
    assert!(llvm_ir.contains("define i32 @main(i32 %argc, i8** %argv)"));
}

#[test]
fn test_talas_sum_classes_parsing() {
    let source = r#"
        class Direction {
            case North
            case South
            case East
            case West
        }
    "#;

    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty(), "Parsing sum classes failed: {:?}", parser.diagnostics);

    let mut checker = TypeChecker::new();
    let is_valid = checker.check_program(&program);
    assert!(is_valid, "Type checking sum classes failed: {:?}", checker.diagnostics);
}

#[test]
fn test_talas_trait_parsing() {
    let source = r#"
        trait Printable {
            printDetails();
            string getHeader() {
                return `=== PRINTABLE ===`
            }
        }

        class User(long id, string name) {
            printDetails() {
                print(`User #${this._id}: ${this._name}`)
            }
        }
    "#;

    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty(), "Parsing trait failed: {:?}", parser.diagnostics);

    let mut checker = TypeChecker::new();
    let is_valid = checker.check_program(&program);
    assert!(is_valid, "Type checking trait failed: {:?}", checker.diagnostics);
}

#[test]
fn test_e2e_person_birthday() {
    let source_code = r#"
        class Person(
            string name,
            var int age
        ) {
            birthday() {
                this._age = this._age + 1
            }
        }

        class Application {
            main() {
                final person = Person(`Lem`, 35)
                print(person.name())
                print(person.age())
                person.birthday()
                print(person.age())
            }
        }
    "#;

    // 1. Lexing & Parsing
    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty(), "Parsing failed: {:?}", parser.diagnostics);

    // 2. Type Checking & Semantic Verification
    let mut checker = TypeChecker::new();
    let is_valid = checker.check_program(&program);
    assert!(is_valid, "Type checking failed: {:?}", checker.diagnostics);

    // 3. IR Lowering to SSA CFG
    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);
    assert_eq!(ir_program.classes.len(), 2);

    // 4. LLVM IR Generation
    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(llvm_ir.contains("%Person = type { i8*, i32 }"));
    assert!(llvm_ir.contains("define void @Person_birthday(%Person* %0)"));
    assert!(llvm_ir.contains("call void @talas_print_str"));
    assert!(llvm_ir.contains("call void @talas_print_i64"));
    assert!(llvm_ir.contains("getelementptr inbounds %Person, %Person*"));
    assert!(llvm_ir.contains("store i32 35"));
    assert!(!llvm_ir.contains("void %"));
}

#[test]
fn test_e2e_trait_polymorphism() {
    let source_code = r#"
        trait Printable {
            printDetails();         // Abstract method

            string getHeader() {    // Default implementation
                return `=== PRINTABLE ITEM ===`
            }
        }

        class User(long id, string name) {
            printDetails() { // Concrete implementation
                print(`User #${this._id}: ${this._name}`)
            }
        }

        class Invoice(long id, decimal total) {
            printDetails() { // Concrete implementation
                print(`Invoice #${this._id}: \$${this._total}`)
            }
        }

        class App {
            render(Printable item) { // Accepts any Printable item
                print(item.getHeader())
                item.printDetails()
            }

            main() {
                final user = User(1000L, `Lem`)
                final invoice = Invoice(5001L, 199.99)

                this.render(user)    // Calls render with a User
                this.render(invoice) // Calls render with an Invoice
            }
        }
    "#;

    // 1. Lexing & Parsing
    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty(), "Parsing failed: {:?}", parser.diagnostics);

    // 2. Type Checking & Semantic Verification
    let mut checker = TypeChecker::new();
    let is_valid = checker.check_program(&program);
    assert!(is_valid, "Type checking failed: {:?}", checker.diagnostics);

    // 3. IR Lowering to SSA CFG
    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);
    assert_eq!(ir_program.classes.len(), 3);

    // 4. LLVM IR Generation
    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(llvm_ir.contains("%User = type { i64, i8* }"));
    assert!(llvm_ir.contains("%Invoice = type { i64, double }"));
    assert!(llvm_ir.contains("define void @App_render_User(%App*"));
    assert!(llvm_ir.contains("define void @App_render_Invoice(%App*"));
    assert!(llvm_ir.contains("call void @User_printDetails(%User*"));
    assert!(llvm_ir.contains("call void @Invoice_printDetails(%Invoice*"));
    assert!(!llvm_ir.contains("void %"));
}

#[test]
fn test_e2e_control_flow_and_dynamic_alloc() {
    let source_code = r#"
        class LargeObject(
            long a,
            long b,
            long c,
            long d,
            long e,
            long f,
            long g,
            long h
        ) {}

        class Application {
            main() {
                final obj = LargeObject(1L, 2L, 3L, 4L, 5L, 6L, 7L, 8L)
                final cond = true

                if (cond) {
                    print(`Condition true`)
                } else {
                    print(`Condition false`)
                }

                for {
                    break
                }
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty(), "Parsing failed: {:?}", parser.diagnostics);

    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program), "Type checking failed: {:?}", checker.diagnostics);

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(llvm_ir.contains("ptrtoint %LargeObject*"));
    assert!(llvm_ir.contains("br i1"));
    assert!(!llvm_ir.contains("call i8* @talas_alloc(i64 32, i64 8)"));
}

#[test]
fn test_e2e_nested_cfg_and_logical_ops() {
    let source_code = r#"
        class Application {
            main() {
                final a = true
                final b = false
                final res = a nand b
                final comp = 10 > 5

                if (a) {
                    if (b) {
                        print(`Inner B`)
                    }
                    print(`Outer A`)
                }

                for {
                    break
                    print(`UNREACHABLE DEAD CODE`)
                }
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty(), "Parsing failed: {:?}", parser.diagnostics);

    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program), "Type checking failed: {:?}", checker.diagnostics);

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(llvm_ir.contains("icmp sgt i32 10, 5") || llvm_ir.contains("icmp sgt i64 10, 5"));
    assert!(llvm_ir.contains("and i1"));
    assert!(llvm_ir.contains("xor i1 %tmp_nand_"));
    assert!(!llvm_ir.contains("UNREACHABLE DEAD CODE"));
}

#[test]
fn test_e2e_default_constructor_args() {
    let source_code = r#"
        class User(
            string name,
            var int age = 0
        ) {}

        class Application {
            main() {
                final user = User(`Lem`)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty(), "Parsing failed: {:?}", parser.diagnostics);

    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program), "Type checking failed: {:?}", checker.diagnostics);
}

#[test]
fn test_e2e_definitely_returns_if_branches() {
    let source_code = r#"
        class Calculator {
            int compute(bool flag) {
                if (flag) {
                    return 10
                } else {
                    return 20
                }
            }
        }

        class Application {
            main() {
                final calc = Calculator()
                print(calc.compute(true))
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty(), "Parsing failed: {:?}", parser.diagnostics);

    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program), "Type checking failed: {:?}", checker.diagnostics);

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(llvm_ir.contains("define i32 @Calculator_compute"));
    assert!(llvm_ir.contains("ret i64 10") || llvm_ir.contains("ret i32 10"));
    assert!(llvm_ir.contains("ret i64 20") || llvm_ir.contains("ret i32 20"));
}

#[test]
fn test_e2e_go_style_zero_values() {
    let source_code = r#"
        class User(
            long id,
            string name,
            var int age = 18
        ) {}

        class Application {
            main() {
                User user
                final u1 = User()
                final u2 = User(1000L)
                final u3 = User(1000L, `Lem`)
                final u4 = User(1000L, `Lem`, 35)

                print(user.id())
                print(user.name())
                print(u1.age())
                print(u2.age())
                print(u3.age())
                print(u4.age())
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty(), "Parsing failed: {:?}", parser.diagnostics);

    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program), "Type checking failed: {:?}", checker.diagnostics);

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(llvm_ir.contains("call i8* @talas_alloc"));
    assert!(llvm_ir.contains("store i32 18"));
    assert!(llvm_ir.contains("store i32 35"));
}

#[test]
fn test_e2e_go_zero_value_user() {
    let source_code = r#"
        class User(
            long id,
            string name,
            var int age = 18
        ) {}

        class Application {
            main() {
                User user

                print(user.id())
                print(user.name())
                print(user.age())
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty(), "Parsing failed: {:?}", parser.diagnostics);

    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program), "Type checking failed: {:?}", checker.diagnostics);

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(llvm_ir.contains("%User = type { i64, i8*, i32 }"));
    assert!(llvm_ir.contains("store i32 18"));
}

#[test]
fn test_e2e_unified_zero_value_instantiation() {
    let source_code = r#"
        class Account(
            long id,
            string holder,
            var int balance = 100
        ) {}

        class Application {
            main() {
                Account a1
                final a2 = Account()
                final a3 = Account(1000L)
                final a4 = Account(1000L, `Lem`)
                final a5 = Account(1000L, `Lem`, 500)

                print(a1.balance())
                print(a2.balance())
                print(a3.balance())
                print(a4.balance())
                print(a5.balance())
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty(), "Parsing failed: {:?}", parser.diagnostics);

    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program), "Type checking failed: {:?}", checker.diagnostics);

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(llvm_ir.contains("store i32 100"));
    assert!(llvm_ir.contains("store i32 500"));
}

#[test]
fn test_type_checker_constructor_argument_types() {
    let source_code = r#"
        class User(long id, string name) {}

        class Application {
            main() {
                final invalid = User(`Lem`, 1000L)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty());

    let mut checker = TypeChecker::new();
    let is_valid = checker.check_program(&program);
    assert!(!is_valid, "Type checker should reject mismatched constructor argument types");
    assert!(checker.diagnostics.iter().any(|d| d.message.contains("Expected") || d.message.contains("argument")));
}

#[test]
fn test_type_checker_variable_decl_type_enforcement() {
    let source_code = r#"
        class Application {
            main() {
                int age = `hello`
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty());

    let mut checker = TypeChecker::new();
    let is_valid = checker.check_program(&program);
    assert!(!is_valid, "Type checker should reject mismatched variable initializer type");
}

#[test]
fn test_type_checker_if_condition_bool_enforcement() {
    let source_code = r#"
        class Application {
            main() {
                if (123) {
                    print(`Hello`)
                }
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty());

    let mut checker = TypeChecker::new();
    let is_valid = checker.check_program(&program);
    assert!(!is_valid, "Type checker should reject non-boolean if condition");
}

#[test]
fn test_e2e_unary_not_lowering() {
    let source_code = r#"
        class Application {
            main() {
                final a = false
                final b = not a
                print(b)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty());

    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program));

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(llvm_ir.contains("xor i1"));
}

#[test]
fn test_e2e_trait_return_type_preservation() {
    let source_code = r#"
        trait Item {
            string getTitle();
        }

        class Book(string title) {
            string getTitle() {
                return this._title
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty());

    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program));

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(llvm_ir.contains("define i8* @Book_getTitle"));
}

#[test]
fn test_e2e_decimal_modulus_lowering() {
    let source_code = r#"
        class Application {
            main() {
                final a = 10.5 % 3.2
                print(a)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    assert!(parser.diagnostics.is_empty());

    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program));

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(llvm_ir.contains("frem double"));
}

#[test]
fn test_e2e_int_and_long_keep_distinct_storage_types() {
    let source_code = r#"
        class Application {
            main() {
                int small = 10
                long big = 10L

                print(small)
                print(big)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking failed: {:?}",
        checker.diagnostics
    );

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    assert!(
        builder.diagnostics.is_empty(),
        "IR building failed: {:?}",
        builder.diagnostics
    );

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    // int must really be i32.
    assert!(
        llvm_ir.contains("store i32 10"),
        "int literal was not lowered as i32:\n{}",
        llvm_ir
    );

    // long must really be i64.
    assert!(
        llvm_ir.contains("store i64 10"),
        "long literal was not lowered as i64:\n{}",
        llvm_ir
    );
}

#[test]
fn test_e2e_reject_wrong_field_assignment_type() {
    let source_code = r#"
        class User(var int age) {
            corrupt() {
                this._age = `not an integer`
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    let valid = checker.check_program(&program);

    assert!(
        !valid,
        "Talas must reject assigning string to int field"
    );

    assert!(
        !checker.diagnostics.is_empty(),
        "Expected assignment type diagnostic"
    );
}

#[test]
fn test_e2e_reject_wrong_method_argument_type() {
    let source_code = r#"
        class Greeter {
            greet(int times) {
                print(`Hello`)
            }
        }

        class Application {
            main() {
                final greeter = Greeter()

                greeter.greet(`wrong`)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    let valid = checker.check_program(&program);

    assert!(
        !valid,
        "Method expects int but string argument was accepted"
    );

    assert!(
        !checker.diagnostics.is_empty(),
        "Expected method argument type diagnostic"
    );
}

#[test]
fn test_e2e_application_constructor_defaults_are_initialized() {
    let source_code = r#"
        class Application(
            var int port = 8080
        ) {
            main() {
                print(this.port())
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking failed: {:?}",
        checker.diagnostics
    );

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    assert!(
        builder.diagnostics.is_empty(),
        "IR building failed: {:?}",
        builder.diagnostics
    );

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    // Synthetic main must initialize Application.port.
    assert!(
        llvm_ir.contains("store i32 8080"),
        "Application default value was never initialized:\n{}",
        llvm_ir
    );
}

#[test]
fn test_e2e_abstract_trait_nonvoid_method_never_returns_void() {
    let source_code = r#"
        trait Item {
            string title();
        }

        class Application {
            main() {}
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking failed: {:?}",
        checker.diagnostics
    );

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    if let Some(start) = llvm_ir.find("define i8* @Item_title") {
        let remaining = &llvm_ir[start..];
        let end = remaining.find("\n}").unwrap_or(remaining.len());
        let trait_function = &remaining[..end];

        assert!(
            !trait_function.contains("ret void"),
            "Non-void trait method generated ret void:\n{}",
            trait_function
        );
    } else {
        assert!(
            llvm_ir.contains("declare i8* @Item_title"),
            "Expected a valid abstract trait declaration:\n{}",
            llvm_ir
        );
    }
}

#[test]
fn test_e2e_reject_invalid_binary_operand_types() {
    let source_code = r#"
        class Application {
            main() {
                final bad = 10 and 20
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    let valid = checker.check_program(&program);

    assert!(
        !valid,
        "'and' must require bool operands"
    );

    assert!(
        !checker.diagnostics.is_empty(),
        "Expected invalid binary operand diagnostic"
    );
}

#[test]
fn test_e2e_string_interpolation_is_not_literal_text() {
    let source_code = r#"
        class Application {
            main() {
                final name = `Lem`

                print(`Hello ${name}`)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking failed: {:?}",
        checker.diagnostics
    );

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    // "${name}" must have been parsed/evaluated,
    // not embedded in the final constant.
    assert!(
        !llvm_ir.contains("${name}"),
        "Interpolation was emitted as literal text:\n{}",
        llvm_ir
    );
}

#[test]
fn test_e2e_unsupported_binary_never_panics_backend() {
    let source_code = r#"
        class Application {
            main() {
                final range = 1 .. 10
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Unexpected semantic failure: {:?}",
        checker.diagnostics
    );

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    // If Range is not implemented yet, a builder diagnostic is valid.
    if !builder.diagnostics.is_empty() {
        return;
    }

    // If the builder accepts it, the LLVM backend must not panic.
    let result = std::panic::catch_unwind(
        std::panic::AssertUnwindSafe(|| {
            let mut emitter = TextualLlvmEmitter::new();
            emitter.emit_program(&ir_program)
        })
    );

    assert!(
        result.is_ok(),
        "LLVM emitter panicked on a parser-supported binary operator"
    );
}

#[test]
fn test_e2e_trait_declared_return_types_preserved_across_all_types() {
    let source_code = r#"
        trait DataSupplier {
            int count();
            long timestamp();
            decimal ratio();
            bool isValid();
            string code();
        }

        class Service(
            int _count,
            long _timestamp,
            decimal _ratio,
            bool _isValid,
            string _code
        ) {
            int count() {
                return this._count
            }
            long timestamp() {
                return this._timestamp
            }
            decimal ratio() {
                return this._ratio
            }
            bool isValid() {
                return this._isValid
            }
            string code() {
                return this._code
            }
        }

        class Application {
            main() {
                final s = Service(42, 1000L, 3.14, true, `OK`)
                print(s.count())
                print(s.timestamp())
                print(s.ratio())
                print(s.isValid())
                print(s.code())
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking failed: {:?}",
        checker.diagnostics
    );

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    assert!(
        builder.diagnostics.is_empty(),
        "IR building failed: {:?}",
        builder.diagnostics
    );

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    // Verify declared return types on functions
    assert!(
        llvm_ir.contains("define i32 @Service_count"),
        "int return type missing:\n{}",
        llvm_ir
    );
    assert!(
        llvm_ir.contains("define i64 @Service_timestamp"),
        "long return type missing:\n{}",
        llvm_ir
    );
    assert!(
        llvm_ir.contains("define double @Service_ratio"),
        "decimal return type missing:\n{}",
        llvm_ir
    );
    assert!(
        llvm_ir.contains("define i1 @Service_isValid"),
        "bool return type missing:\n{}",
        llvm_ir
    );
    assert!(
        llvm_ir.contains("define i8* @Service_code"),
        "string return type missing:\n{}",
        llvm_ir
    );
}

#[test]
fn test_e2e_range_expression_emits_graceful_diagnostic() {
    let source_code = r#"
        class Application {
            main() {
                final numbers = 1 .. 10
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    let mut checker = TypeChecker::new();
    let _checked = checker.check_program(&program);

    let mut builder = IrBuilder::new();
    let _ir_program = builder.build_program(&program);

    assert!(
        !parser.diagnostics.is_empty() || !checker.diagnostics.is_empty() || !builder.diagnostics.is_empty(),
        "Expected range expression '..' to produce diagnostic error"
    );
}

#[test]
fn test_e2e_bare_unsupported_function_call_emits_diagnostic() {
    let source_code = r#"
        class Application {
            main() {
                doSomething()
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    let mut checker = TypeChecker::new();
    let _checked = checker.check_program(&program);

    let mut builder = IrBuilder::new();
    let _ir_program = builder.build_program(&program);

    assert!(
        !parser.diagnostics.is_empty() || !checker.diagnostics.is_empty() || !builder.diagnostics.is_empty(),
        "Expected bare unsupported call 'doSomething()' to produce diagnostic error"
    );
}

#[test]
fn test_e2e_unsupported_member_access_emits_diagnostic() {
    let source_code = r#"
        class Application {
            main() {
                final x = 42
                print(x.invalid_member)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    let mut checker = TypeChecker::new();
    let _checked = checker.check_program(&program);

    let mut builder = IrBuilder::new();
    let _ir_program = builder.build_program(&program);

    assert!(
        !parser.diagnostics.is_empty() || !checker.diagnostics.is_empty() || !builder.diagnostics.is_empty(),
        "Expected unresolvable member access to produce diagnostic error"
    );
}

#[test]
fn test_e2e_invalid_assignment_target_emits_diagnostic() {
    let source_code = r#"
        class Application {
            main() {
                (10 + 20) = 100
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    let mut checker = TypeChecker::new();
    let _checked = checker.check_program(&program);

    let mut builder = IrBuilder::new();
    let _ir_program = builder.build_program(&program);

    assert!(
        !parser.diagnostics.is_empty() || !checker.diagnostics.is_empty() || !builder.diagnostics.is_empty(),
        "Expected invalid assignment target '(10 + 20) = 100' to produce diagnostic error"
    );
}

#[test]
fn test_e2e_optional_chaining_emits_diagnostic_until_supported() {
    let source_code = r#"
        class User(var string name) {}

        class Application {
            main() {
                final u = User(`Alice`)
                print(u?.name)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    let mut checker = TypeChecker::new();
    let _checked = checker.check_program(&program);

    let mut builder = IrBuilder::new();
    let _ir_program = builder.build_program(&program);

    assert!(
        !parser.diagnostics.is_empty() || !checker.diagnostics.is_empty() || !builder.diagnostics.is_empty(),
        "Expected optional member access '?.' to produce diagnostic error until supported"
    );
}

#[test]
fn test_e2e_break_and_continue_outside_loop_emits_diagnostic() {
    let break_source = r#"
        class Application {
            main() {
                break
            }
        }
    "#;

    let lexer1 = Lexer::new(break_source);
    let mut parser1 = Parser::new(lexer1);
    let program1 = parser1.parse_program();
    let mut checker1 = TypeChecker::new();
    checker1.check_program(&program1);

    assert!(
        !checker1.diagnostics.is_empty(),
        "Expected 'break' outside loop to produce diagnostic error"
    );

    let continue_source = r#"
        class Application {
            main() {
                continue
            }
        }
    "#;

    let lexer2 = Lexer::new(continue_source);
    let mut parser2 = Parser::new(lexer2);
    let program2 = parser2.parse_program();
    let mut checker2 = TypeChecker::new();
    checker2.check_program(&program2);

    assert!(
        !checker2.diagnostics.is_empty(),
        "Expected 'continue' outside loop to produce diagnostic error"
    );
}

#[test]
fn test_e2e_invalid_unary_operands_emit_diagnostic() {
    let not_int_source = r#"
        class Application {
            main() {
                final x = not 123
            }
        }
    "#;

    let lexer1 = Lexer::new(not_int_source);
    let mut parser1 = Parser::new(lexer1);
    let program1 = parser1.parse_program();
    let mut checker1 = TypeChecker::new();
    checker1.check_program(&program1);

    assert!(
        !checker1.diagnostics.is_empty(),
        "Expected 'not 123' to produce diagnostic error"
    );

    let neg_bool_source = r#"
        class Application {
            main() {
                final y = -true
            }
        }
    "#;

    let lexer2 = Lexer::new(neg_bool_source);
    let mut parser2 = Parser::new(lexer2);
    let program2 = parser2.parse_program();
    let mut checker2 = TypeChecker::new();
    checker2.check_program(&program2);

    assert!(
        !checker2.diagnostics.is_empty(),
        "Expected '-true' to produce diagnostic error"
    );
}

#[test]
fn test_e2e_guard_check_no_error_flow() {
    let source_code = r#"
        class Application {
            main() {
                guard {
                    print(`Step 1`)
                }
                check(err) {
                    if (err) {
                        print(`Error occurred`)
                    }
                    print(`Cleanup`)
                }
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking failed: {:?}",
        checker.diagnostics
    );

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    assert!(
        builder.diagnostics.is_empty(),
        "IR building failed: {:?}",
        builder.diagnostics
    );

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(
        llvm_ir.contains("define i32 @main"),
        "LLVM emission failed for guard check no-error flow"
    );
}

#[test]
fn test_e2e_guard_raise_check_error_flow() {
    let source_code = r#"
        class CustomError(string _message) {
            string message() {
                return this._message
            }
        }

        class Application {
            main() {
                guard {
                    print(`Before raise`)
                    raise(CustomError(`Something went wrong`))
                    print(`Unreachable after raise`)
                }
                check(err) {
                    if (err) {
                        print(`Handled error`)
                    }
                    print(`Cleanup always runs`)
                }
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking failed: {:?}",
        checker.diagnostics
    );

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    assert!(
        builder.diagnostics.is_empty(),
        "IR building failed: {:?}",
        builder.diagnostics
    );

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(
        llvm_ir.contains("define i32 @main"),
        "LLVM emission failed for guard raise check error flow"
    );
}

#[test]
fn test_e2e_raise_outside_guard_emits_diagnostic() {
    let source_code = r#"
        class Application {
            main() {
                raise(`Error outside guard`)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    let mut checker = TypeChecker::new();
    checker.check_program(&program);

    assert!(
        !checker.diagnostics.is_empty(),
        "Expected 'raise' outside guard block to produce diagnostic error"
    );
}

#[test]
fn test_e2e_guard_raise_string_interpolation_flow() {
    let source_code = r#"
        class App {
            main() {
                guard {
                    raise(`an error occurred`)
                }
                check(error) {
                    if (error) {
                        print(`${error}`)
                    }
                }
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking failed: {:?}",
        checker.diagnostics
    );

    let mut builder = IrBuilder::new();
    let ir_program = builder.build_program(&program);

    assert!(
        builder.diagnostics.is_empty(),
        "IR building failed: {:?}",
        builder.diagnostics
    );

    let mut emitter = TextualLlvmEmitter::new();
    let llvm_ir = emitter.emit_program(&ir_program);

    assert!(
        llvm_ir.contains("define i32 @main"),
        "LLVM emission failed for guard raise string interpolation flow"
    );
}

#[test]
fn test_e2e_raise_primitive_emits_diagnostic() {
    let source_code = r#"
        class Application {
            main() {
                guard {
                    raise(123)
                }
                check(err) {
                    print(`Cleanup`)
                }
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    let mut checker = TypeChecker::new();
    let is_valid = checker.check_program(&program);

    assert!(
        !is_valid,
        "Type checker must reject raising primitive numbers"
    );

    assert!(
        checker.diagnostics.iter().any(|d| d.message.contains("raise target must be a string or satisfy 'Error'")),
        "Expected primitive raise diagnostic, got: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_raise_custom_class_error_trait() {
    let source_code = r#"
        class CustomError(int code, string rawMsg) {
            string message() {
                return `Error ${this._code}: ${this._rawMsg}`
            }
        }

        class Application {
            main() {
                guard {
                    raise(CustomError(404, `Not Found`))
                }
                check(err) {
                    if (err) {
                        print(err.message())
                    }
                }
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking custom error class failed: {:?}",
        checker.diagnostics
    );

    let mut builder = IrBuilder::new();
    let _ir_program = builder.build_program(&program);

    assert!(
        builder.diagnostics.is_empty(),
        "IR building custom error class failed: {:?}",
        builder.diagnostics
    );
}

#[test]
fn test_e2e_list_literal_indexing_and_methods() {
    let source_code = r#"
        class Application {
            main() {
                final numbers = [10, 20, 30]
                numbers.add(40)
                print(numbers[0])
                print(numbers.len())
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing list literal failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking list literal failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_map_literal_indexing_and_len() {
    let source_code = r#"
        class Application {
            main() {
                final scores = [1000L: 95, 1001L: 88]
                print(scores[1000L])
                print(scores.len())
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing map literal failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking map literal failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_list_index_out_of_bounds_raises_error() {
    let source_code = r#"
        class Application {
            main() {
                guard {
                    final list = [1, 2]
                    print(list[10])
                }
                check(err) {
                    if (err) {
                        print(err.message())
                    }
                }
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing list out of bounds failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking list out of bounds failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_async_spawn_and_await() {
    let source_code = r#"
        class Application {
            async fetchScore(int id) {
                return 95
            }

            main() {
                final task = this.fetchScore(1000)
                final score = await task
                print(score)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing async fetchScore failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking async fetchScore failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_async_task_raise_rethrown_on_await() {
    let source_code = r#"
        class Application {
            async riskyTask() {
                raise(`async task error`)
            }

            main() {
                guard {
                    final task = this.riskyTask()
                    await task
                }
                check(err) {
                    if (err) {
                        print(err.message())
                    }
                }
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing async riskyTask failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking async riskyTask failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_channel_send_and_receive() {
    let source_code = r#"
        class Application {
            main() {
                final ch = Channel()
                ch.send(100)
                final val = ch.receive()
                print(val)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing channel send and receive failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking channel send and receive failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_async_expression_spawn() {
    let source_code = r#"
        class Application {
            main() {
                final task = async (10 + 20)
                final val = await task
                print(val)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing inline async expression failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking inline async expression failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_multiple_async_tasks_concurrent_await() {
    let source_code = r#"
        class Application {
            async compute(int val) {
                return val * 2
            }

            main() {
                final t1 = this.compute(10)
                final t2 = this.compute(20)
                final r1 = await t1
                final r2 = await t2
                print(r1 + r2)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing multiple concurrent async tasks failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking multiple concurrent async tasks failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_guard_check_inside_async_method_body() {
    let source_code = r#"
        class Application {
            async safeTask() {
                guard {
                    raise(`inner async error`)
                }
                check(err) {
                    if (err) {
                        return 42
                    }
                }
                return 0
            }

            main() {
                final task = this.safeTask()
                final res = await task
                print(res)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing guard/check inside async method failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking guard/check inside async method failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_task_all_combinator_success() {
    let source_code = r#"
        class Application {
            main() {
                final tasks = [async 10, async 20]
                final results = await Task.all(tasks)
                print(results.len())
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing Task.all failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking Task.all failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_task_all_combinator_fail_fast() {
    let source_code = r#"
        class Application {
            async failingTask() {
                raise(`task 2 failed`)
            }

            main() {
                guard {
                    final tasks = [async 10, this.failingTask()]
                    await Task.all(tasks)
                }
                check(err) {
                    if (err) {
                        print(err.message())
                    }
                }
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing Task.all fail-fast failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking Task.all fail-fast failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_task_any_and_race_combinators() {
    let source_code = r#"
        class Application {
            main() {
                final t1 = async 100
                final t2 = async 200
                final winner = await Task.race([t1, t2])
                print(winner)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing Task.race failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking Task.race failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_task_all_settled_combinator() {
    let source_code = r#"
        class Application {
            main() {
                final tasks = [async 10, async 20]
                final settled = await Task.allSettled(tasks)
                print(settled.len())
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing Task.allSettled failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking Task.allSettled failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_await_and_cancel_timeout() {
    let source_code = r#"
        class Application {
            main() {
                guard {
                    final task = async 500
                    final res = awaitAndCancel(task, 2000)
                    print(res)
                }
                check(err) {
                    if (err) {
                        print(err.message())
                    }
                }
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing awaitAndCancel failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking awaitAndCancel failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_task_async_static_factory() {
    let source_code = r#"
        class Application {
            main() {
                final task = Task.async(10 + 20)
                final val = await task
                print(val)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing Task.async failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking Task.async failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_datetime_factories_and_getters() {
    let source_code = r#"
        class Application {
            main() {
                final dt = DateTime.of(2026, 8, 31, 14, 30)
                print(dt.year())
                print(dt.month())
                print(dt.day())
                print(dt.hour())
                print(dt.minute())
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing DateTime factories failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking DateTime factories failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_datetime_arithmetic_and_diff() {
    let source_code = r#"
        class Application {
            main() {
                final start = DateTime.of(2026, 8, 31, 12, 0)
                final later = start.addHours(2)
                print(later.hour())
                final diffSec = later.diffSeconds(start)
                print(diffSec)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing DateTime arithmetic failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking DateTime arithmetic failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_datetime_formatting_and_parsing() {
    let source_code = r#"
        class Application {
            main() {
                final dt = DateTime.parse(`2026-08-31T14:30:00+08:00`)
                final formatted = dt.format(`yyyy-MM-dd HH:mm:ss`)
                print(formatted)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing DateTime formatting and parsing failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking DateTime formatting and parsing failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_duration_and_addition() {
    let source_code = r#"
        class Application {
            main() {
                final start = DateTime.of(2026, 8, 31, 10, 0)
                final dur = Duration.hours(3)
                final end = start.addDuration(dur)
                print(end.hour())
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing Duration addition failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking Duration addition failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_datetime_instant_timezone_invariance() {
    let source_code = r#"
        class Application {
            main() {
                final manila = DateTime.of(2026, 8, 31, 17, 0, `Asia/Manila`)
                final tokyo = manila.in(`Asia/Tokyo`)
                print(manila.timestamp() == tokyo.timestamp())
                print(manila.hour())
                print(tokyo.hour())
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing timezone invariance failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking timezone invariance failed: {:?}",
        checker.diagnostics
    );
}

#[test]
fn test_e2e_datetime_unix_timestamp_factories() {
    let source_code = r#"
        class Application {
            main() {
                final dt = DateTime.fromUnix(1788159600)
                final sec = dt.unix()
                final ms = dt.unixMillis()
                print(sec)
                print(ms)
            }
        }
    "#;

    let lexer = Lexer::new(source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    assert!(
        parser.diagnostics.is_empty(),
        "Parsing Unix timestamp factories failed: {:?}",
        parser.diagnostics
    );

    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&program),
        "Type checking Unix timestamp factories failed: {:?}",
        checker.diagnostics
    );
}
