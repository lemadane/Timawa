use std::env;
use std::fs;
use std::process::{self, Command};
use talas_compiler::{DiagnosticEmitter, IrBuilder, Lexer, Parser, TextualLlvmEmitter, TypeChecker};

fn print_usage() {
    eprintln!("Talas Language Compiler v0.1.0");
    eprintln!("Usage: talas <command> <source.talas> [options]");
    eprintln!("Commands:");
    eprintln!("  check <file.talas>                  Check source code for syntax and semantic errors");
    eprintln!("  emit-ir <file.talas>                Emit custom Typed Talas IR representation");
    eprintln!("  emit-llvm <file.talas> [-o <out>]   Emit textual LLVM IR (.ll) file");
    eprintln!("  build <file.talas> [--release] [-o <out>] Compile source file to native executable");
    eprintln!("  run <file.talas>                    Compile and execute source file immediately");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        print_usage();
        process::exit(1);
    }

    let command = &args[1];
    let mut file_path = String::new();
    let mut out_binary = String::new();
    let mut is_release = false;

    let mut i = 2;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--release" {
            is_release = true;
        } else if arg == "-o" && i + 1 < args.len() {
            i += 1;
            out_binary = args[i].clone();
        } else if !arg.starts_with('-') && file_path.is_empty() {
            file_path = arg.clone();
        }
        i += 1;
    }

    if file_path.is_empty() {
        eprintln!("Error: No source file specified.");
        print_usage();
        process::exit(1);
    }

    if out_binary.is_empty() {
        let base = file_path.trim_end_matches(".talas").trim_end_matches(".tl");
        out_binary = base.to_string();
    }

    let source_code = match fs::read_to_string(&file_path) {
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
            let ir_program = builder.build_program(&program);
            println!("{:#?}", ir_program);
        }
        "emit-llvm" => {
            let mut builder = IrBuilder::new();
            let ir_program = builder.build_program(&program);

            let mut emitter = TextualLlvmEmitter::new();
            let llvm_ir = emitter.emit_program(&ir_program);

            if !out_binary.is_empty() && out_binary != file_path.trim_end_matches(".talas").trim_end_matches(".tl") {
                if let Err(e) = fs::write(&out_binary, &llvm_ir) {
                    eprintln!("Error writing LLVM IR file '{}': {}", out_binary, e);
                    process::exit(1);
                }
                println!("LLVM IR successfully written to '{}'", out_binary);
            } else {
                println!("{}", llvm_ir);
            }
        }
        "build" => {
            let mut builder = IrBuilder::new();
            let ir_program = builder.build_program(&program);

            let mut emitter = TextualLlvmEmitter::new();
            let llvm_ir = emitter.emit_program(&ir_program);

            let base_name = file_path.trim_end_matches(".talas").trim_end_matches(".tl");
            let ll_path = format!("{}.ll", base_name);

            if let Err(e) = fs::write(&ll_path, &llvm_ir) {
                eprintln!("Error writing temporary LLVM IR file '{}': {}", ll_path, e);
                process::exit(1);
            }

            println!("LLVM IR generated at '{}'", ll_path);

            let mut clang_cmd = Command::new("clang");
            clang_cmd.arg(&ll_path).arg("-o").arg(&out_binary);

            if is_release {
                clang_cmd.arg("-O3").arg("-flto");
                println!("Building in release mode (-O3, -flto)...");
            }

            let status = clang_cmd.status();

            match status {
                Ok(s) if s.success() => {
                    println!("Successfully built native binary: './{}'", out_binary);
                }
                _ => {
                    println!("LLVM IR generated at '{}'. To link manually:", ll_path);
                    println!("  clang {} -o {}", ll_path, out_binary);
                }
            }
        }
        "run" => {
            let mut builder = IrBuilder::new();
            let ir_program = builder.build_program(&program);

            let mut emitter = TextualLlvmEmitter::new();
            let llvm_ir = emitter.emit_program(&ir_program);

            let tmp_dir = env::temp_dir();
            let ll_path = tmp_dir.join("talas_temp_app.ll");
            let bin_path = tmp_dir.join("talas_temp_app");

            if let Err(e) = fs::write(&ll_path, &llvm_ir) {
                eprintln!("Error writing temporary IR: {}", e);
                process::exit(1);
            }

            let compile_status = Command::new("clang")
                .arg(&ll_path)
                .arg("-o")
                .arg(&bin_path)
                .status();

            match compile_status {
                Ok(s) if s.success() => {
                    let run_status = Command::new(&bin_path).status();
                    let _ = fs::remove_file(&ll_path);
                    let _ = fs::remove_file(&bin_path);
                    if let Ok(exit_code) = run_status {
                        process::exit(exit_code.code().unwrap_or(0));
                    }
                }
                _ => {
                    eprintln!("Failed to compile temporary executable with clang.");
                    let _ = fs::remove_file(&ll_path);
                    process::exit(1);
                }
            }
        }
        other => {
            eprintln!("Unknown command '{}'", other);
            print_usage();
            process::exit(1);
        }
    }
}
