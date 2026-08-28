use crate::ir::nodes::*;
use crate::parser::BinaryOp;
use crate::types::Type;
use std::collections::{HashMap, HashSet};

pub struct TextualLlvmEmitter {
    output: String,
    string_constants: HashMap<String, String>,
    string_counter: usize,
}

impl TextualLlvmEmitter {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            string_constants: HashMap::new(),
            string_counter: 0,
        }
    }

    pub fn emit_program(&mut self, ir_program: &IrProgram) -> String {
        self.output.clear();

        // Target Data Layout
        self.output.push_str("; Target Data Layout for Talas Native Binary\n");
        self.output.push_str("target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128\"\n\n");

        // External runtime declarations
        self.output.push_str("declare void @talas_runtime_init()\n");
        self.output.push_str("declare void @talas_runtime_cleanup()\n");
        self.output.push_str("declare i8* @talas_alloc(i64, i64)\n");
        self.output.push_str("declare void @talas_print_str(i8*)\n");
        self.output.push_str("declare void @talas_print_i64(i64)\n");
        self.output.push_str("declare void @talas_print_f64(double)\n");
        self.output.push_str("declare void @talas_print_bool(i1)\n\n");

        let mut declared_types: HashSet<String> = ir_program.classes.iter().map(|c| c.name.clone()).collect();

        // Object layouts for concrete classes
        for cls in &ir_program.classes {
            let field_types = cls
                .fields
                .iter()
                .map(|(_, ty)| self.type_to_llvm(ty))
                .collect::<Vec<_>>()
                .join(", ");
            self.output.push_str(&format!("%{} = type {{ {} }}\n", cls.name, field_types));
        }

        // Object layouts for abstract trait types referenced in parameters/receivers
        for func in &ir_program.functions {
            for (_, ty) in &func.params {
                if let Type::Class { name } = ty {
                    if !declared_types.contains(name) {
                        self.output.push_str(&format!("%{} = type {{ }}\n", name));
                        declared_types.insert(name.clone());
                    }
                }
            }
        }
        self.output.push('\n');

        // Emit functions
        for func in &ir_program.functions {
            let func_str = self.emit_function(func);
            self.output.push_str(&func_str);
            self.output.push('\n');
        }

        // Emit synthetic C-ABI main entry point if any class contains a main() method
        let main_fn = ir_program
            .functions
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case("main"));

        if let Some(m_fn) = main_fn {
            let target_symbol = format!("{}_{}", m_fn.class_name, m_fn.name);
            let class_type = format!("%{}", m_fn.class_name);
            self.output.push_str("define i32 @main(i32 %argc, i8** %argv) {\n");
            self.output.push_str("entry:\n");
            self.output.push_str("  call void @talas_runtime_init()\n");
            self.output.push_str("  %raw_mem = call i8* @talas_alloc(i64 32, i64 8)\n");
            self.output.push_str(&format!("  %app = bitcast i8* %raw_mem to {}*\n", class_type));
            self.output.push_str(&format!("  call void @{}({}* %app)\n", target_symbol, class_type));
            self.output.push_str("  call void @talas_runtime_cleanup()\n");
            self.output.push_str("  ret i32 0\n");
            self.output.push_str("}\n\n");
        }

        // Global string constants
        let constants = self.string_constants.clone();
        for (label, str_val) in &constants {
            let bytes_len = str_val.as_bytes().len() + 1;
            let escaped = self.escape_llvm_string(str_val);
            self.output.push_str(&format!(
                "@{} = private unnamed_addr constant [{} x i8] c\"{}\", align 1\n",
                label,
                bytes_len,
                escaped
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
            IrInstruction::InstantiateClass { dest, class_name: _, .. } => {
                format!("%{} = call i8* @talas_alloc(i64 32, i64 8)", dest.0)
            }
            IrInstruction::CallMethod { dest, class_name, method_name, receiver, args, ret_ty } => {
                let fn_symbol = format!("{}_{}", class_name, method_name);
                let ret_ty_str = self.type_to_llvm(ret_ty);
                let rx_str = self.emit_operand(receiver);
                let rx_ty_str = self.type_to_llvm(&receiver.get_type());

                let formatted_args = args
                    .iter()
                    .map(|a| format!("{} {}", self.type_to_llvm(&a.get_type()), self.emit_operand(a)))
                    .collect::<Vec<_>>();

                let all_args = if formatted_args.is_empty() {
                    format!("{} {}", rx_ty_str, rx_str)
                } else {
                    format!("{} {}, {}", rx_ty_str, rx_str, formatted_args.join(", "))
                };

                if ret_ty == &Type::Void || ret_ty_str == "void" {
                    format!("call void @{}({})", fn_symbol, all_args)
                } else if let Some(r) = dest {
                    format!("%{} = call {} @{}({})", r.0, ret_ty_str, fn_symbol, all_args)
                } else {
                    format!("call {} @{}({})", ret_ty_str, fn_symbol, all_args)
                }
            }
            IrInstruction::CallRuntime { dest, symbol, args, ret_ty } => {
                let ret_ty_str = self.type_to_llvm(ret_ty);
                let args_str = args.iter().map(|a| format!("{} {}", self.type_to_llvm(&a.get_type()), self.emit_operand(a))).collect::<Vec<_>>().join(", ");
                if ret_ty == &Type::Void || ret_ty_str == "void" {
                    format!("call void @{}({})", symbol, args_str)
                } else if let Some(r) = dest {
                    format!("%{} = call {} @{}({})", r.0, ret_ty_str, symbol, args_str)
                } else {
                    format!("call {} @{}({})", ret_ty_str, symbol, args_str)
                }
            }
            IrInstruction::Branch { cond, then_block, else_block } => {
                let cond_str = self.emit_operand(cond);
                format!("br i1 {}, label %{}, label %{}", cond_str, then_block, else_block)
            }
            IrInstruction::Jump { target } => {
                format!("br label %{}", target)
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
            IrOperand::Register(reg, _) => format!("%{}", reg.0),
            IrOperand::ConstantInt(v) => v.to_string(),
            IrOperand::ConstantDecimal(v) => format!("{:.6}", v),
            IrOperand::ConstantString(v) => {
                let label = format!("str_{}", self.string_counter);
                self.string_counter += 1;
                self.string_constants.insert(label.clone(), v.clone());
                format!("getelementptr inbounds ([{} x i8], [{} x i8]* @{}, i32 0, i32 0)", v.as_bytes().len() + 1, v.as_bytes().len() + 1, label)
            }
            IrOperand::ConstantBool(v) => (if *v { "true" } else { "false" }).to_string(),
            IrOperand::Null => "null".to_string(),
        }
    }

    fn type_to_llvm(&self, ty: &Type) -> String {
        match ty {
            Type::Int => "i32".to_string(),
            Type::Decimal => "double".to_string(),
            Type::Byte => "i8".to_string(),
            Type::Long => "i64".to_string(),
            Type::Bool => "i1".to_string(),
            Type::String => "i8*".to_string(),
            Type::Void => "void".to_string(),
            Type::Class { name } => format!("%{}*", name),
            Type::Nullable(inner) => self.type_to_llvm(inner),
            Type::AnonymousObject { .. } => "i8*".to_string(),
            Type::Function { .. } => "i8*".to_string(),
        }
    }
}
