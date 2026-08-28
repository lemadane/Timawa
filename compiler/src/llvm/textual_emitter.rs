use crate::ir::*;
use crate::parser::BinaryOp;
use crate::types::Type;
use std::collections::HashSet;

pub struct TextualLlvmEmitter {
    output: String,
    string_constants: Vec<(String, String)>,
    declared_externs: HashSet<String>,
}

impl TextualLlvmEmitter {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            string_constants: Vec::new(),
            declared_externs: HashSet::new(),
        }
    }

    fn type_to_llvm(&self, ty: &Type) -> String {
        match ty {
            Type::Int => "i32".into(),
            Type::Decimal => "double".into(),
            Type::Byte => "i8".into(),
            Type::Long => "i64".into(),
            Type::Bool => "i1".into(),
            Type::String => "i8*".into(),
            Type::Void => "void".into(),
            Type::Class { name } => format!("%{}*", name),
            Type::Nullable(inner) => self.type_to_llvm(inner),
            Type::AnonymousObject { .. } => "i8*".into(),
            Type::Function { .. } => "i8*".into(),
        }
    }

    pub fn emit_program(&mut self, ir_program: &IrProgram) -> String {
        self.output.clear();
        self.string_constants.clear();
        self.declared_externs.clear();

        self.output.push_str("; ModuleID = 'talas_module'\n");
        self.output.push_str("source_filename = \"talas\"\n\n");

        // Runtime symbols
        self.output.push_str("declare void @talas_runtime_init()\n");
        self.output.push_str("declare void @talas_runtime_cleanup()\n");
        self.output.push_str("declare i8* @talas_alloc(i64, i64)\n");
        self.output.push_str("declare void @talas_print_str(i8*)\n\n");

        // Emit class struct definitions
        for cls in &ir_program.classes {
            let field_types = cls
                .fields
                .iter()
                .map(|(_, ty)| self.type_to_llvm(ty))
                .collect::<Vec<_>>()
                .join(", ");
            self.output.push_str(&format!("%{} = type {{ {} }}\n", cls.name, field_types));
        }
        self.output.push('\n');

        // Emit functions
        for func in &ir_program.functions {
            self.output.push_str(&self.emit_function(func));
            self.output.push('\n');
        }

        // Emit synthetic C-ABI main entry point if Application.main exists
        let has_app_main = ir_program.functions.iter().any(|f| f.class_name == "Application" && f.name == "main");
        if has_app_main {
            self.output.push_str("define i32 @main(i32 %argc, i8** %argv) {\n");
            self.output.push_str("entry:\n");
            self.output.push_str("  call void @talas_runtime_init()\n");
            self.output.push_str("  %raw_mem = call i8* @talas_alloc(i64 32, i64 8)\n");
            self.output.push_str("  %app = bitcast i8* %raw_mem to %Application*\n");
            self.output.push_str("  call void @Application_main(%Application* %app)\n");
            self.output.push_str("  call void @talas_runtime_cleanup()\n");
            self.output.push_str("  ret i32 0\n");
            self.output.push_str("}\n\n");
        }

        // Global string constants
        for (label, str_val) in &self.string_constants {
            let bytes_len = str_val.as_bytes().len() + 1;
            self.output.push_str(&format!(
                "@{} = private unnamed_addr constant [{} x i8] c\"{}\", align 1\n",
                label,
                bytes_len,
                self.escape_llvm_string(str_val)
            ));
        }

        self.output.clone()
    }

    fn escape_llvm_string(&self, s: &str) -> String {
        let mut escaped = String::new();
        for b in s.as_bytes() {
            match b {
                b'\\' => escaped.push_str("\\5C"),
                b'"' => escaped.push_str("\\22"),
                b'\n' => escaped.push_str("\\0A"),
                b'\r' => escaped.push_str("\\0D"),
                b'\t' => escaped.push_str("\\09"),
                32..=126 => escaped.push(*b as char),
                _ => escaped.push_str(&format!("\\{:02X}", b)),
            }
        }
        escaped.push_str("\\00");
        escaped
    }

    fn emit_function(&mut self, func: &IrFunction) -> String {
        let mut buf = String::new();
        let ret_ty_str = self.type_to_llvm(&func.return_type);
        let fn_name = format!("{}_{}", func.class_name, func.name);
        let receiver_str = format!("{} %{}", self.type_to_llvm(&func.receiver.1), func.receiver.0.0);

        let other_params = func
            .params
            .iter()
            .map(|(reg, ty)| format!("{} %{}", self.type_to_llvm(ty), reg.0))
            .collect::<Vec<_>>();

        let all_params = if other_params.is_empty() {
            receiver_str
        } else {
            format!("{}, {}", receiver_str, other_params.join(", "))
        };

        buf.push_str(&format!("define {} @{}({}) {{\n", ret_ty_str, fn_name, all_params));

        for block in &func.blocks {
            buf.push_str(&format!("{}:\n", block.id));
            for instr in &block.instructions {
                buf.push_str(&format!("  {}\n", self.emit_instruction(instr)));
            }
        }

        buf.push_str("}\n");
        buf
    }

    fn emit_instruction(&mut self, instr: &IrInstruction) -> String {
        match instr {
            IrInstruction::Alloca { dest, ty } => {
                let ty_str = self.type_to_llvm(ty);
                format!("%{} = alloca {}, align 8", dest.0, ty_str)
            }
            IrInstruction::Load { dest, ptr, ty } => {
                let ty_str = self.type_to_llvm(ty);
                format!("%{} = load {}, {}* %{}, align 8", dest.0, ty_str, ty_str, ptr.0)
            }
            IrInstruction::Store { val, ptr } => {
                let val_ty = val.get_type();
                let ty_str = self.type_to_llvm(&val_ty);
                let val_str = self.emit_operand(val);
                format!("store {} {}, {}* %{}, align 8", ty_str, val_str, ty_str, ptr.0)
            }
            IrInstruction::Binary { dest, op, left, right, ty } => {
                let ty_str = self.type_to_llvm(ty);
                let l_str = self.emit_operand(left);
                let r_str = self.emit_operand(right);
                let op_str = match op {
                    BinaryOp::Add => if ty == &Type::Decimal { "fadd" } else { "add" },
                    BinaryOp::Sub => if ty == &Type::Decimal { "fsub" } else { "sub" },
                    BinaryOp::Mul => if ty == &Type::Decimal { "fmul" } else { "mul" },
                    BinaryOp::Div => if ty == &Type::Decimal { "fdiv" } else { "sdiv" },
                    BinaryOp::Mod => "srem",
                    BinaryOp::Equal => "icmp eq",
                    BinaryOp::NotEqual => "icmp ne",
                    BinaryOp::Less => "icmp slt",
                    BinaryOp::LessEqual => "icmp sle",
                    BinaryOp::Greater => "icmp sgt",
                    BinaryOp::GreaterEqual => "icmp sge",
                    BinaryOp::And => "and",
                    BinaryOp::Or => "or",
                    _ => "add",
                };
                format!("%{} = {} {} {}, {}", dest.0, op_str, ty_str, l_str, r_str)
            }
            IrInstruction::InstantiateClass { dest, class_name, .. } => {
                format!("%{} = call i8* @talas_alloc(i64 32, i64 8)", dest.0)
            }
            IrInstruction::CallMethod { dest, class_name, method_name, receiver, args, ret_ty } => {
                let fn_symbol = format!("{}_{}", class_name, method_name);
                let ret_ty_str = self.type_to_llvm(ret_ty);
                let rx_str = self.emit_operand(receiver);
                if let Some(r) = dest {
                    format!("%{} = call {} @{}({} {})", r.0, ret_ty_str, fn_symbol, self.type_to_llvm(&receiver.get_type()), rx_str)
                } else {
                    format!("call {} @{}({} {})", ret_ty_str, fn_symbol, self.type_to_llvm(&receiver.get_type()), rx_str)
                }
            }
            IrInstruction::CallRuntime { dest, symbol, args, ret_ty } => {
                let ret_ty_str = self.type_to_llvm(ret_ty);
                let args_str = args.iter().map(|a| format!("{} {}", self.type_to_llvm(&a.get_type()), self.emit_operand(a))).collect::<Vec<_>>().join(", ");
                if let Some(r) = dest {
                    format!("%{} = call {} @{}({})", r.0, ret_ty_str, symbol, args_str)
                } else {
                    format!("call {} @{}({})", ret_ty_str, symbol, args_str)
                }
            }
            IrInstruction::Jump { target } => {
                format!("br label %{}", target)
            }
            IrInstruction::Branch { cond, then_block, else_block } => {
                let cond_str = self.emit_operand(cond);
                format!("br i1 {}, label %{}, label %{}", cond_str, then_block, else_block)
            }
            IrInstruction::Return { val } => {
                if let Some(v) = val {
                    format!("ret {} {}", self.type_to_llvm(&v.get_type()), self.emit_operand(v))
                } else {
                    "ret void".to_string()
                }
            }
        }
    }

    fn emit_operand(&mut self, op: &IrOperand) -> String {
        match op {
            IrOperand::ConstantInt(v) => v.to_string(),
            IrOperand::ConstantDecimal(v) => format!("{:.6}", v),
            IrOperand::ConstantBool(v) => (if *v { 1 } else { 0 }).to_string(),
            IrOperand::Null => "null".to_string(),
            IrOperand::ConstantString(s) => {
                let label = format!(".str.{}", self.string_constants.len());
                self.string_constants.push((label.clone(), s.clone()));
                let bytes_len = s.as_bytes().len() + 1;
                format!("getelementptr inbounds ([{} x i8], [{} x i8]* @{}, i64 0, i64 0)", bytes_len, bytes_len, label)
            }
            IrOperand::Register(r, _) => format!("%{}", r.0),
        }
    }
}
