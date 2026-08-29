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
    assert!(llvm_ir.contains("define void @Person_birthday(%Person* %v0)"));
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

    assert!(llvm_ir.contains("icmp sgt i64 10, 5"));
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
    assert!(llvm_ir.contains("ret i32 10"));
    assert!(llvm_ir.contains("ret i32 20"));
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
                final u2 = User(`Lem`)
                final u3 = User(`Lem`, 35)

                print(user.id())
                print(user.name())
                print(u1.age())
                print(u2.age())
                print(u3.age())
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
