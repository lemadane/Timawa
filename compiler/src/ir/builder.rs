use super::nodes::*;
use crate::parser::*;
use crate::types::Type;
use std::collections::HashMap;

pub struct IrBuilder {
    next_reg: usize,
    next_block: usize,
    current_instructions: Vec<IrInstruction>,
    blocks: Vec<BasicBlock>,
    var_pointers: Vec<HashMap<String, (Register, Type)>>,
}

impl IrBuilder {
    pub fn new() -> Self {
        Self {
            next_reg: 0,
            next_block: 0,
            current_instructions: Vec::new(),
            blocks: Vec::new(),
            var_pointers: Vec::new(),
        }
    }

    fn fresh_reg(&mut self) -> Register {
        let r = Register(self.next_reg);
        self.next_reg += 1;
        r
    }

    fn fresh_block(&mut self) -> BlockId {
        let b = BlockId(self.next_block);
        self.next_block += 1;
        b
    }

    fn emit(&mut self, instr: IrInstruction) {
        self.current_instructions.push(instr);
    }

    fn finish_block(&mut self, id: BlockId) {
        let instrs = std::mem::take(&mut self.current_instructions);
        self.blocks.push(BasicBlock { id, instructions: instrs });
    }

    fn push_var_scope(&mut self) {
        self.var_pointers.push(HashMap::new());
    }

    fn pop_var_scope(&mut self) {
        self.var_pointers.pop();
    }

    fn insert_var(&mut self, name: String, reg: Register, ty: Type) {
        if let Some(scope) = self.var_pointers.last_mut() {
            scope.insert(name, (reg, ty));
        }
    }

    fn lookup_var(&self, name: &str) -> Option<(Register, Type)> {
        for scope in self.var_pointers.iter().rev() {
            if let Some(entry) = scope.get(name) {
                return Some(entry.clone());
            }
        }
        None
    }

    pub fn build_program(&mut self, program: &Program) -> IrProgram {
        let mut ir_classes = Vec::new();
        let mut ir_functions = Vec::new();

        for item in &program.items {
            if let Item::Class(cls) = item {
                let (ir_cls, fns) = self.build_class(cls);
                ir_classes.push(ir_cls);
                ir_functions.extend(fns);
            }
        }

        IrProgram {
            classes: ir_classes,
            functions: ir_functions,
        }
    }

    fn build_class(&mut self, cls: &ClassDecl) -> (IrClass, Vec<IrFunction>) {
        let mut fields = Vec::new();

        for param in &cls.primary_params {
            let field_name = format!("_{}", param.name);
            let field_ty = match &param.type_spec {
                TypeSpec::Int => Type::Int,
                TypeSpec::Decimal => Type::Decimal,
                TypeSpec::Byte => Type::Byte,
                TypeSpec::Long => Type::Long,
                TypeSpec::Bool => Type::Bool,
                TypeSpec::String => Type::String,
                TypeSpec::Class(n) => Type::Class { name: n.clone() },
                TypeSpec::Nullable(_) => Type::Nullable(Box::new(Type::Void)),
            };
            fields.push((field_name, field_ty));
        }

        let ir_cls = IrClass {
            name: cls.name.clone(),
            fields,
        };

        let mut functions = Vec::new();

        for param in &cls.primary_params {
            let getter_fn = self.build_getter(cls, param);
            functions.push(getter_fn);
        }

        for method in &cls.methods {
            let ir_fn = self.build_method(cls, method);
            functions.push(ir_fn);
        }

        (ir_cls, functions)
    }

    fn build_getter(&mut self, cls: &ClassDecl, param: &ConstructorParam) -> IrFunction {
        self.next_reg = 0;
        self.next_block = 0;
        self.blocks.clear();
        self.current_instructions.clear();
        self.var_pointers.clear();

        let entry_block = self.fresh_block();
        let receiver_reg = self.fresh_reg();
        let this_ty = Type::Class { name: cls.name.clone() };

        let field_ty = match &param.type_spec {
            TypeSpec::Int => Type::Int,
            TypeSpec::Decimal => Type::Decimal,
            TypeSpec::Byte => Type::Byte,
            TypeSpec::Long => Type::Long,
            TypeSpec::Bool => Type::Bool,
            TypeSpec::String => Type::String,
            TypeSpec::Class(n) => Type::Class { name: n.clone() },
            TypeSpec::Nullable(_) => Type::Nullable(Box::new(Type::Void)),
        };

        let load_reg = self.fresh_reg();
        self.emit(IrInstruction::Load {
            dest: load_reg,
            ptr: receiver_reg,
            ty: field_ty.clone(),
        });
        self.emit(IrInstruction::Return {
            val: Some(IrOperand::Register(load_reg, field_ty.clone())),
        });

        self.finish_block(entry_block);

        IrFunction {
            class_name: cls.name.clone(),
            name: param.name.clone(),
            receiver: (receiver_reg, this_ty),
            params: Vec::new(),
            return_type: field_ty,
            blocks: std::mem::take(&mut self.blocks),
        }
    }

    fn build_method(&mut self, cls: &ClassDecl, method: &MethodDecl) -> IrFunction {
        self.next_reg = 0;
        self.next_block = 0;
        self.blocks.clear();
        self.current_instructions.clear();
        self.var_pointers.clear();

        self.push_var_scope();
        let entry_block = self.fresh_block();
        let receiver_reg = self.fresh_reg();
        let this_ty = Type::Class { name: cls.name.clone() };

        let this_ptr = self.fresh_reg();
        self.emit(IrInstruction::Alloca { dest: this_ptr, ty: this_ty.clone() });
        self.emit(IrInstruction::Store {
            val: IrOperand::Register(receiver_reg, this_ty.clone()),
            ptr: this_ptr,
        });
        self.insert_var("this".into(), this_ptr, this_ty.clone());

        let mut ir_params = Vec::new();
        for p in &method.params {
            let p_reg = self.fresh_reg();
            let p_ty = match &p.type_spec {
                TypeSpec::Int => Type::Int,
                TypeSpec::Decimal => Type::Decimal,
                TypeSpec::Byte => Type::Byte,
                TypeSpec::Long => Type::Long,
                TypeSpec::Bool => Type::Bool,
                TypeSpec::String => Type::String,
                TypeSpec::Class(n) => Type::Class { name: n.clone() },
                TypeSpec::Nullable(_) => Type::Nullable(Box::new(Type::Void)),
            };
            ir_params.push((p_reg, p_ty.clone()));

            let ptr_reg = self.fresh_reg();
            self.emit(IrInstruction::Alloca { dest: ptr_reg, ty: p_ty.clone() });
            self.emit(IrInstruction::Store { val: IrOperand::Register(p_reg, p_ty.clone()), ptr: ptr_reg });
            self.insert_var(p.name.clone(), ptr_reg, p_ty);
        }

        for stmt in &method.body {
            self.build_stmt(stmt);
        }

        let ret_ty = match &method.return_type {
            None => Type::Void,
            Some(TypeSpec::Int) => Type::Int,
            Some(TypeSpec::Decimal) => Type::Decimal,
            Some(TypeSpec::Long) => Type::Long,
            Some(TypeSpec::Bool) => Type::Bool,
            Some(TypeSpec::String) => Type::String,
            Some(TypeSpec::Class(n)) => Type::Class { name: n.clone() },
            Some(TypeSpec::Byte) => Type::Byte,
            Some(TypeSpec::Nullable(_)) => Type::Nullable(Box::new(Type::Void)),
        };

        if ret_ty == Type::Void {
            let is_terminated = self.current_instructions.last().map_or(false, |i| matches!(i, IrInstruction::Return { .. }));
            if !is_terminated {
                self.emit(IrInstruction::Return { val: None });
            }
        }

        self.finish_block(entry_block);
        self.pop_var_scope();

        IrFunction {
            class_name: cls.name.clone(),
            name: method.name.clone(),
            receiver: (receiver_reg, this_ty),
            params: ir_params,
            return_type: ret_ty,
            blocks: std::mem::take(&mut self.blocks),
        }
    }

    fn build_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::LocalDecl { name, initializer, .. } => {
                let init_op = if let Some(init) = initializer {
                    self.build_expr(init)
                } else {
                    IrOperand::ConstantInt(0)
                };
                let ty = init_op.get_type();
                let ptr_reg = self.fresh_reg();
                self.emit(IrInstruction::Alloca { dest: ptr_reg, ty: ty.clone() });
                self.emit(IrInstruction::Store { val: init_op, ptr: ptr_reg });
                self.insert_var(name.clone(), ptr_reg, ty);
            }
            StmtKind::Expr(expr) => {
                self.build_expr(expr);
            }
            StmtKind::If { condition, then_branch, else_branch } => {
                let cond_op = self.build_expr(condition);
                let then_block = self.fresh_block();
                let else_block = self.fresh_block();
                let merge_block = self.fresh_block();

                self.emit(IrInstruction::Branch {
                    cond: cond_op,
                    then_block,
                    else_block: if else_branch.is_some() { else_block } else { merge_block },
                });

                self.push_var_scope();
                for s in then_branch {
                    self.build_stmt(s);
                }
                self.emit(IrInstruction::Jump { target: merge_block });
                self.finish_block(then_block);
                self.pop_var_scope();

                if let Some(eb) = else_branch {
                    self.push_var_scope();
                    for s in eb {
                        self.build_stmt(s);
                    }
                    self.emit(IrInstruction::Jump { target: merge_block });
                    self.finish_block(else_block);
                    self.pop_var_scope();
                }
            }
            StmtKind::For { condition_or_iterable, body, .. } => {
                let header_block = self.fresh_block();
                let body_block = self.fresh_block();
                let exit_block = self.fresh_block();

                self.emit(IrInstruction::Jump { target: header_block });

                let cond_op = if let Some(cond) = condition_or_iterable {
                    self.build_expr(cond)
                } else {
                    IrOperand::ConstantBool(true)
                };

                self.emit(IrInstruction::Branch {
                    cond: cond_op,
                    then_block: body_block,
                    else_block: exit_block,
                });
                self.finish_block(header_block);

                self.push_var_scope();
                for s in body {
                    self.build_stmt(s);
                }
                self.emit(IrInstruction::Jump { target: header_block });
                self.finish_block(body_block);
                self.pop_var_scope();
            }
            StmtKind::Break => {}
            StmtKind::Continue => {}
            StmtKind::Return(expr_opt) => {
                let val = expr_opt.as_ref().map(|e| self.build_expr(e));
                self.emit(IrInstruction::Return { val });
            }
            StmtKind::Yield(expr) => {
                let val = self.build_expr(expr);
                self.emit(IrInstruction::Return { val: Some(val) });
            }
        }
    }

    fn build_expr(&mut self, expr: &Expr) -> IrOperand {
        match &expr.kind {
            ExprKind::Print { expr: inner } => {
                let op = self.build_expr(inner);
                let symbol = match op.get_type() {
                    Type::String => "talas_print_str",
                    Type::Int => "talas_print_i64",
                    Type::Long => "talas_print_i64",
                    Type::Decimal => "talas_print_f64",
                    Type::Bool => "talas_print_bool",
                    _ => "talas_print_str",
                };
                self.emit(IrInstruction::CallRuntime {
                    dest: None,
                    symbol: symbol.into(),
                    args: vec![op],
                    ret_ty: Type::Void,
                });
                IrOperand::ConstantInt(0)
            }
            ExprKind::IntLiteral(v) => IrOperand::ConstantInt(*v),
            ExprKind::DecimalLiteral(v) => IrOperand::ConstantDecimal(*v),
            ExprKind::StringLiteral(v) => IrOperand::ConstantString(v.clone()),
            ExprKind::BoolLiteral(v) => IrOperand::ConstantBool(*v),
            ExprKind::NullLiteral => IrOperand::Null,
            ExprKind::This => {
                let (ptr, ty) = self.lookup_var("this").expect("'this' pointer missing");
                let dest = self.fresh_reg();
                self.emit(IrInstruction::Load { dest, ptr, ty: ty.clone() });
                IrOperand::Register(dest, ty)
            }
            ExprKind::Identifier(name) => {
                let (ptr, ty) = self.lookup_var(name).expect("Variable missing in IR");
                let dest = self.fresh_reg();
                self.emit(IrInstruction::Load { dest, ptr, ty: ty.clone() });
                IrOperand::Register(dest, ty)
            }
            ExprKind::ClassInstantiation { class_name, args } => {
                let arg_ops = args.iter().map(|a| self.build_expr(a)).collect();
                let dest = self.fresh_reg();
                self.emit(IrInstruction::InstantiateClass {
                    dest,
                    class_name: class_name.clone(),
                    args: arg_ops,
                });
                IrOperand::Register(dest, Type::Class { name: class_name.clone() })
            }
            ExprKind::Call { callee, args } => {
                let arg_ops = args.iter().map(|a| self.build_expr(a)).collect();
                match &callee.kind {
                    ExprKind::MemberAccess { object, member, .. } => {
                        let obj_op = self.build_expr(object);
                        let dest = self.fresh_reg();
                        self.emit(IrInstruction::CallMethod {
                            dest: Some(dest),
                            class_name: "User".into(),
                            method_name: member.clone(),
                            receiver: obj_op,
                            args: arg_ops,
                            ret_ty: Type::Void,
                        });
                        IrOperand::Register(dest, Type::Void)
                    }
                    _ => IrOperand::ConstantInt(0),
                }
            }
            ExprKind::Binary { op, left, right } => {
                let l_op = self.build_expr(left);
                let r_op = self.build_expr(right);
                let dest = self.fresh_reg();
                let ty = l_op.get_type();
                self.emit(IrInstruction::Binary {
                    dest,
                    op: op.clone(),
                    left: l_op,
                    right: r_op,
                    ty: ty.clone(),
                });
                IrOperand::Register(dest, ty)
            }
            _ => IrOperand::ConstantInt(0),
        }
    }
}
