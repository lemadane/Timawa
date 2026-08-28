use std::env;
use std::fs;
use std::process;
use talas_compiler::{DiagnosticEmitter, IrBuilder, Lexer, Parser, TextualLlvmEmitter, TypeChecker};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Talas Language Compiler v0.1.0");
        eprintln!("Usage: talas-compiler <command> <source.tl> [options]");
        eprintln!("Commands:");
        eprintln!("  check <file.tl>            Check source code for syntax and semantic errors");
        eprintln!("  emit-ir <file.tl>          Emit custom Typed Talas IR representation");
        eprintln!("  emit-llvm <file.tl> [out]  Emit textual LLVM IR (.ll) file");
        eprintln!("  build <file.tl> [out]      Compile source file down to native executable");
        process::exit(1);
    }

    let command = &args[1];
    let file_path = &args[2];

    let source_code = match fs::read_to_string(file_path) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("Error reading source file '{}': {}", file_path, err);
            process::exit(1);
        }
    };

    let lexer = Lexer::new(&source_code);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();

    if !parser.diagnostics.is_empty() {
        DiagnosticEmitter::emit(&parser.diagnostics, &source_code);
        process::exit(1);
    }

    let mut checker = TypeChecker::new();
    let typed_program = checker.check_program(&program);

    if !checker.diagnostics.is_empty() {
        DiagnosticEmitter::emit(&checker.diagnostics, &source_code);
        process::exit(1);
    }

    match command.as_str() {
        "check" => {
            println!("Check successful! 0 errors.");
        }
        "emit-ir" => {
            let mut builder = IrBuilder::new();
            let ir_program = builder.build_program(&typed_program);
            println!("{:#?}", ir_program);
        }
        "emit-llvm" => {
            let mut builder = IrBuilder::new();
            let ir_program = builder.build_program(&typed_program);

            let mut emitter = TextualLlvmEmitter::new();
            let llvm_ir = emitter.emit_program(&ir_program);

            if args.len() >= 4 {
                let out_path = &args[3];
                if let Err(e) = fs::write(out_path, &llvm_ir) {
                    eprintln!("Error writing LLVM IR file '{}': {}", out_path, e);
                    process::exit(1);
                }
                println!("LLVM IR successfully written to '{}'", out_path);
            } else {
                println!("{}", llvm_ir);
            }
        }
        "build" => {
            let mut builder = IrBuilder::new();
            let ir_program = builder.build_program(&typed_program);

            let mut emitter = TextualLlvmEmitter::new();
            let llvm_ir = emitter.emit_program(&ir_program);

            let ll_path = format!("{}.ll", file_path.trim_end_matches(".tl"));
            if let Err(e) = fs::write(&ll_path, &llvm_ir) {
                eprintln!("Error writing temporary LLVM IR file '{}': {}", ll_path, e);
                process::exit(1);
            }
            println!("LLVM IR generated at '{}'", ll_path);
            println!("To compile to native binary using clang/gcc:");
            println!("  clang {} -ltalas_runtime -o output_executable", ll_path);
        }
        other => {
            eprintln!("Unknown command '{}'", other);
            process::exit(1);
        }
    }
}
