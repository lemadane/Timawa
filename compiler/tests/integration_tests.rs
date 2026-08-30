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
