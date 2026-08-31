use super::nodes::*;
use crate::diagnostics::Diagnostic;
use crate::parser::*;
use crate::types::Type;
use std::collections::HashMap;

pub struct IrBuilder {
    next_reg: usize,
    next_block: usize,
    current_instructions: Vec<IrInstruction>,
    blocks: Vec<BasicBlock>,
    var_pointers: Vec<HashMap<String, (Register, Type)>>,
    method_return_types: HashMap<(String, String), Type>,
    class_fields: HashMap<(String, String), Type>,
    class_field_indices: HashMap<(String, String), (usize, Type)>,
    class_decls: HashMap<String, ClassDecl>,
    loop_stack: Vec<(BlockId, BlockId)>,
    guard_stack: Vec<(Register, BlockId)>,
    current_block_id: Option<BlockId>,
    pub diagnostics: Vec<Diagnostic>,
}

impl IrBuilder {
    pub fn new() -> Self {
        Self {
            next_reg: 0,
            next_block: 0,
            current_instructions: Vec::new(),
            blocks: Vec::new(),
            var_pointers: Vec::new(),
            method_return_types: HashMap::new(),
            class_fields: HashMap::new(),
            class_field_indices: HashMap::new(),
            class_decls: HashMap::new(),
            loop_stack: Vec::new(),
            guard_stack: Vec::new(),
            current_block_id: None,
            diagnostics: Vec::new(),
        }
    }

    fn type_spec_to_type(&self, ts: &TypeSpec) -> Type {
        match ts {
            TypeSpec::Int => Type::Int,
            TypeSpec::Decimal => Type::Decimal,
            TypeSpec::Byte => Type::Byte,
            TypeSpec::Long => Type::Long,
            TypeSpec::Bool => Type::Bool,
            TypeSpec::String => Type::String,
            TypeSpec::Class(n) => {
                if n == "void" {
                    Type::Void
                } else {
                    Type::Class { name: n.clone() }
                }
            }
            TypeSpec::Nullable(inner) => Type::Nullable(Box::new(self.type_spec_to_type(inner))),
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

    fn is_current_terminated(&self) -> bool {
        self.current_instructions.last().map_or(false, |i| {
            matches!(i, IrInstruction::Return { .. } | IrInstruction::Jump { .. } | IrInstruction::Branch { .. })
        })
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
        self.method_return_types.clear();
        self.class_fields.clear();
        self.class_field_indices.clear();
        self.class_decls.clear();

        for item in &program.items {
            if let Item::Class(cls) = item {
                self.class_decls.insert(cls.name.clone(), cls.clone());
            }
        }

        let mut ir_classes = Vec::new();
        let mut ir_functions = Vec::new();

        // 1. Collect all concrete classes
        let mut concrete_classes = Vec::new();
        for item in &program.items {
            if let Item::Class(cls) = item {
                concrete_classes.push(cls.clone());
            }
        }

        // 2. Pre-register method return types and class field types & indices
        for item in &program.items {
            match item {
                Item::Class(cls) => {
                    for (i, param) in cls.primary_params.iter().enumerate() {
                        let field_ty = self.type_spec_to_type(&param.type_spec);
                        self.class_fields.insert((cls.name.clone(), format!("_{}", param.name)), field_ty.clone());
                        self.class_fields.insert((cls.name.clone(), param.name.clone()), field_ty.clone());
                        self.class_field_indices.insert((cls.name.clone(), format!("_{}", param.name)), (i, field_ty.clone()));
                        self.class_field_indices.insert((cls.name.clone(), param.name.clone()), (i, field_ty.clone()));
                        self.method_return_types.insert((cls.name.clone(), param.name.clone()), field_ty);
                    }
                    for case in &cls.cases {
                        self.method_return_types.insert((cls.name.clone(), case.name.clone()), Type::Class { name: cls.name.clone() });
                    }
                    for method in &cls.methods {
                        let ret_ty = method.return_type.as_ref().map_or(Type::Void, |t| self.type_spec_to_type(t));
                        self.method_return_types.insert((cls.name.clone(), method.name.clone()), ret_ty.clone());

                        let has_trait_param = method.params.iter().any(|p| matches!(p.type_spec, TypeSpec::Class(_)));
                        if has_trait_param {
                            for target_cls in &concrete_classes {
                                if target_cls.name != cls.name {
                                    let spec_name = format!("{}_{}", method.name, target_cls.name);
                                    self.method_return_types.insert((cls.name.clone(), spec_name), ret_ty.clone());
                                }
                            }
                        }
                    }
                }
                Item::Trait(tr) => {
                    for method in &tr.methods {
                        let ret_ty = method.return_type.as_ref().map_or(Type::Void, |t| self.type_spec_to_type(t));
                        self.method_return_types.insert((tr.name.clone(), method.name.clone()), ret_ty.clone());
                        for cls in &concrete_classes {
                            self.method_return_types
                                .entry((cls.name.clone(), method.name.clone()))
                                .or_insert_with(|| ret_ty.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        // 3. Build classes and traits
        for item in &program.items {
            match item {
                Item::Class(cls) => {
                    let (ir_cls, fns) = self.build_class(cls, &concrete_classes);
                    ir_classes.push(ir_cls);
                    ir_functions.extend(fns);
                }
                Item::Trait(tr) => {
                    let dummy_cls = ClassDecl {
                        name: tr.name.clone(),
                        primary_params: Vec::new(),
                        cases: Vec::new(),
                        methods: tr.methods.clone(),
                        span: tr.span,
                    };
                    for method in &tr.methods {
                        let ir_fn = self.build_trait_method(&dummy_cls, method, &concrete_classes);
                        ir_functions.push(ir_fn);
                    }
                }
                _ => {}
            }
        }

        IrProgram {
            classes: ir_classes,
            functions: ir_functions,
        }
    }

    fn build_class(&mut self, cls: &ClassDecl, concrete_classes: &[ClassDecl]) -> (IrClass, Vec<IrFunction>) {
        let mut fields = Vec::new();

        for param in &cls.primary_params {
            let field_name = format!("_{}", param.name);
            let field_ty = self.type_spec_to_type(&param.type_spec);
            fields.push((field_name, field_ty));
        }

        let ir_cls = IrClass {
            name: cls.name.clone(),
            fields,
        };

        let mut functions = Vec::new();

        for (i, param) in cls.primary_params.iter().enumerate() {
            let getter_fn = self.build_getter(cls, param, i);
            functions.push(getter_fn);
        }

        for method in &cls.methods {
            let has_trait_param = method.params.iter().any(|p| matches!(p.type_spec, TypeSpec::Class(_)));
            if has_trait_param {
                // Monomorphize method for each concrete class
                for target_cls in concrete_classes {
                    if target_cls.name != cls.name {
                        let mut specialized_method = method.clone();
                        specialized_method.name = format!("{}_{}", method.name, target_cls.name);
                        for p in &mut specialized_method.params {
                            if matches!(p.type_spec, TypeSpec::Class(_)) {
                                p.type_spec = TypeSpec::Class(target_cls.name.clone());
                            }
                        }
                        let spec_fn = self.build_method(cls, &specialized_method);
                        functions.push(spec_fn);
                    }
                }
            }
            let ir_fn = self.build_method(cls, method);
            functions.push(ir_fn);
        }

        (ir_cls, functions)
    }

    fn build_getter(&mut self, cls: &ClassDecl, param: &ConstructorParam, field_index: usize) -> IrFunction {
        self.next_reg = 0;
        self.next_block = 0;
        self.blocks.clear();
        self.current_instructions.clear();
        self.var_pointers.clear();

        let entry_block = self.fresh_block();
        let receiver_reg = self.fresh_reg();
        let this_ty = Type::Class { name: cls.name.clone() };

        let field_ty = self.type_spec_to_type(&param.type_spec);

        let load_reg = self.fresh_reg();
        self.emit(IrInstruction::GetField {
            dest: load_reg,
            ptr: receiver_reg,
            class_name: cls.name.clone(),
            field_index,
            field_ty: field_ty.clone(),
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

    fn build_trait_method(&mut self, cls: &ClassDecl, method: &MethodDecl, _concrete_classes: &[ClassDecl]) -> IrFunction {
        if !method.body.is_empty() {
            return self.build_method(cls, method);
        }

        self.next_reg = 0;
        self.next_block = 0;
        self.blocks.clear();
        self.current_instructions.clear();
        self.var_pointers.clear();

        let entry_block = self.fresh_block();
        let receiver_reg = self.fresh_reg();
        let this_ty = Type::Class { name: cls.name.clone() };

        let mut param_info = Vec::new();
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
            param_info.push((p.name.clone(), p_reg, p_ty));
        }

        let ret_ty = method.return_type.as_ref().map_or(Type::Void, |t| self.type_spec_to_type(t));
        let ret_val = if ret_ty == Type::Void {
            None
        } else {
            Some(IrOperand::Null)
        };
        self.emit(IrInstruction::Return { val: ret_val });
        self.finish_block(entry_block);

        IrFunction {
            class_name: cls.name.clone(),
            name: method.name.clone(),
            receiver: (receiver_reg, this_ty),
            params: ir_params,
            return_type: ret_ty,
            blocks: std::mem::take(&mut self.blocks),
        }
    }

    fn build_method(&mut self, cls: &ClassDecl, method: &MethodDecl) -> IrFunction {
        self.next_reg = 0;
        self.next_block = 0;
        self.blocks.clear();
        self.current_instructions.clear();
        self.var_pointers.clear();
        self.loop_stack.clear();

        self.push_var_scope();
        let entry_block = self.fresh_block();
        self.current_block_id = Some(entry_block);

        let receiver_reg = self.fresh_reg();
        let this_ty = Type::Class { name: cls.name.clone() };

        let mut param_info = Vec::new();
        let mut ir_params = Vec::new();

        for p in &method.params {
            let p_reg = self.fresh_reg();
            let p_ty = self.type_spec_to_type(&p.type_spec);
            ir_params.push((p_reg, p_ty.clone()));
            param_info.push((p.name.clone(), p_reg, p_ty));
        }

        let this_ptr = self.fresh_reg();
        self.emit(IrInstruction::Alloca { dest: this_ptr, ty: this_ty.clone() });
        self.emit(IrInstruction::Store {
            val: IrOperand::Register(receiver_reg, this_ty.clone()),
            ptr: this_ptr,
        });
        self.insert_var("this".into(), this_ptr, this_ty.clone());

        for (p_name, p_reg, p_ty) in param_info {
            let ptr_reg = self.fresh_reg();
            self.emit(IrInstruction::Alloca { dest: ptr_reg, ty: p_ty.clone() });
            self.emit(IrInstruction::Store { val: IrOperand::Register(p_reg, p_ty.clone()), ptr: ptr_reg });
            self.insert_var(p_name, ptr_reg, p_ty);
        }

        for stmt in &method.body {
            self.build_stmt(stmt);
            if self.is_current_terminated() {
                break;
            }
        }

        let ret_ty = method.return_type.as_ref().map_or(Type::Void, |t| self.type_spec_to_type(t));

        if !self.is_current_terminated() {
            self.emit(IrInstruction::Return { val: None });
        }

        if let Some(b_id) = self.current_block_id {
            self.finish_block(b_id);
        }

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

    pub fn build_class_instantiation(&mut self, class_name: &str, args: &[Expr]) -> IrOperand {
        if class_name == "Channel" {
            let dest = self.fresh_reg();
            self.emit(IrInstruction::CallRuntime {
                dest: Some(dest),
                symbol: "talas_channel_create".into(),
                args: vec![],
                ret_ty: Type::Class { name: "Channel".into() },
            });
            return IrOperand::Register(dest, Type::Class { name: "Channel".into() });
        }
        let mut arg_ops: Vec<IrOperand> = args.iter().map(|a| self.build_expr(a)).collect();
        if let Some(cls_decl) = self.class_decls.get(class_name).cloned() {
            if arg_ops.len() < cls_decl.primary_params.len() {
                for param in cls_decl.primary_params.iter().skip(arg_ops.len()) {
                    if let Some(def_expr) = &param.default_value {
                        arg_ops.push(self.build_expr(def_expr));
                    } else {
                        let default_op = match self.type_spec_to_type(&param.type_spec) {
                            Type::Int | Type::Long | Type::Byte => IrOperand::ConstantInt(0),
                            Type::Decimal => IrOperand::ConstantDecimal(0.0),
                            Type::Bool => IrOperand::ConstantBool(false),
                            Type::String => IrOperand::ConstantString("".into()),
                            Type::Class { name: ref cname } => {
                                self.build_class_instantiation(cname, &[])
                            }
                            _ => IrOperand::Null,
                        };
                        arg_ops.push(default_op);
                    }
                }
            }
        }
        let dest = self.fresh_reg();
        self.emit(IrInstruction::InstantiateClass {
            dest,
            class_name: class_name.to_string(),
            args: arg_ops,
        });
        IrOperand::Register(dest, Type::Class { name: class_name.to_string() })
    }

    fn build_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::LocalDecl { name, initializer, type_annotation, .. } => {
                let init_op = if let Some(init) = initializer {
                    self.build_expr(init)
                } else if let Some(ts) = type_annotation {
                    let ty = self.type_spec_to_type(ts);
                    match ty {
                        Type::Int | Type::Long | Type::Byte => IrOperand::ConstantInt(0),
                        Type::Decimal => IrOperand::ConstantDecimal(0.0),
                        Type::Bool => IrOperand::ConstantBool(false),
                        Type::String => IrOperand::ConstantString("".into()),
                        Type::Class { name: ref cname } => {
                            self.build_class_instantiation(cname, &[])
                        }
                        _ => IrOperand::Null,
                    }
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
                let mut cond_op = self.build_expr(condition);
                let cond_ty = cond_op.get_type();
                if cond_ty != Type::Bool {
                    let dest = self.fresh_reg();
                    let (op_code, rhs) = match cond_ty {
                        Type::Decimal => (BinaryOp::NotEqual, IrOperand::ConstantDecimal(0.0)),
                        Type::Int | Type::Long => (BinaryOp::NotEqual, IrOperand::ConstantInt(0)),
                        _ => (BinaryOp::NotEqual, IrOperand::Null),
                    };
                    self.emit(IrInstruction::Binary {
                        dest,
                        op: op_code,
                        left: cond_op.clone(),
                        right: rhs,
                        operand_ty: cond_ty,
                        result_ty: Type::Bool,
                    });
                    cond_op = IrOperand::Register(dest, Type::Bool);
                }
                let then_block = self.fresh_block();
                let else_block = self.fresh_block();
                let merge_block = self.fresh_block();

                if let Some(cb) = self.current_block_id {
                    self.emit(IrInstruction::Branch {
                        cond: cond_op,
                        then_block,
                        else_block: if else_branch.is_some() { else_block } else { merge_block },
                    });
                    self.finish_block(cb);
                }

                self.current_block_id = Some(then_block);
                self.push_var_scope();
                for s in then_branch {
                    self.build_stmt(s);
                    if self.is_current_terminated() {
                        break;
                    }
                }
                if !self.is_current_terminated() {
                    self.emit(IrInstruction::Jump { target: merge_block });
                }
                if let Some(cb) = self.current_block_id {
                    self.finish_block(cb);
                }
                self.pop_var_scope();

                if let Some(eb) = else_branch {
                    self.current_block_id = Some(else_block);
                    self.push_var_scope();
                    for s in eb {
                        self.build_stmt(s);
                        if self.is_current_terminated() {
                            break;
                        }
                    }
                    if !self.is_current_terminated() {
                        self.emit(IrInstruction::Jump { target: merge_block });
                    }
                    if let Some(cb) = self.current_block_id {
                        self.finish_block(cb);
                    }
                    self.pop_var_scope();
                }

                self.current_block_id = Some(merge_block);
            }
            StmtKind::For { condition_or_iterable, body, .. } => {
                let header_block = self.fresh_block();
                let body_block = self.fresh_block();
                let exit_block = self.fresh_block();

                self.emit(IrInstruction::Jump { target: header_block });
                if let Some(cb) = self.current_block_id {
                    self.finish_block(cb);
                }

                self.current_block_id = Some(header_block);
                let cond_op = if let Some(cond) = condition_or_iterable {
                    let mut op = self.build_expr(cond);
                    let cond_ty = op.get_type();
                    if cond_ty != Type::Bool {
                        let dest = self.fresh_reg();
                        let (op_code, rhs) = match cond_ty {
                            Type::Decimal => (BinaryOp::NotEqual, IrOperand::ConstantDecimal(0.0)),
                            Type::Int | Type::Long => (BinaryOp::NotEqual, IrOperand::ConstantInt(0)),
                            _ => (BinaryOp::NotEqual, IrOperand::Null),
                        };
                        self.emit(IrInstruction::Binary {
                            dest,
                            op: op_code,
                            left: op.clone(),
                            right: rhs,
                            operand_ty: cond_ty,
                            result_ty: Type::Bool,
                        });
                        op = IrOperand::Register(dest, Type::Bool);
                    }
                    op
                } else {
                    IrOperand::ConstantBool(true)
                };

                self.emit(IrInstruction::Branch {
                    cond: cond_op,
                    then_block: body_block,
                    else_block: exit_block,
                });
                self.finish_block(header_block);

                self.current_block_id = Some(body_block);
                self.loop_stack.push((header_block, exit_block));
                self.push_var_scope();
                for s in body {
                    self.build_stmt(s);
                    if self.is_current_terminated() {
                        break;
                    }
                }
                if !self.is_current_terminated() {
                    self.emit(IrInstruction::Jump { target: header_block });
                }
                if let Some(cb) = self.current_block_id {
                    self.finish_block(cb);
                }
                self.pop_var_scope();
                self.loop_stack.pop();

                self.current_block_id = Some(exit_block);
            }
            StmtKind::Break => {
                if let Some((_, exit_target)) = self.loop_stack.last().cloned() {
                    self.emit(IrInstruction::Jump { target: exit_target });
                }
            }
            StmtKind::Continue => {
                if let Some((header_target, _)) = self.loop_stack.last().cloned() {
                    self.emit(IrInstruction::Jump { target: header_target });
                }
            }
            StmtKind::Return(expr_opt) => {
                let val = expr_opt.as_ref().map(|e| self.build_expr(e));
                self.emit(IrInstruction::Return { val });
            }
            StmtKind::Yield(expr) => {
                let val = self.build_expr(expr);
                self.emit(IrInstruction::Return { val: Some(val) });
            }
            StmtKind::Guard { body, error_var, check_body } => {
                let err_ty = Type::Nullable(Box::new(Type::Class { name: "Error".into() }));
                let err_ptr = self.fresh_reg();
                self.emit(IrInstruction::Alloca { dest: err_ptr, ty: err_ty.clone() });
                self.emit(IrInstruction::Store { val: IrOperand::Null, ptr: err_ptr });

                let check_block = self.fresh_block();
                self.guard_stack.push((err_ptr, check_block));

                self.push_var_scope();
                for s in body {
                    self.build_stmt(s);
                    if self.is_current_terminated() {
                        break;
                    }
                }
                if !self.is_current_terminated() {
                    self.emit(IrInstruction::Jump { target: check_block });
                }
                if let Some(cb) = self.current_block_id {
                    self.finish_block(cb);
                }
                self.pop_var_scope();
                self.guard_stack.pop();

                self.current_block_id = Some(check_block);
                self.push_var_scope();
                self.insert_var(error_var.clone(), err_ptr, err_ty);
                for s in check_body {
                    self.build_stmt(s);
                    if self.is_current_terminated() {
                        break;
                    }
                }
                self.pop_var_scope();
            }
            StmtKind::Raise(expr) => {
                let mut val_op = self.build_expr(expr);
                if val_op.get_type() == Type::String {
                    val_op = self.build_class_instantiation("GenericError", &[expr.clone()]);
                }
                if let Some((err_ptr, check_target)) = self.guard_stack.last().cloned() {
                    self.emit(IrInstruction::Store { val: val_op, ptr: err_ptr });
                    self.emit(IrInstruction::Jump { target: check_target });
                } else {
                    self.diagnostics.push(Diagnostic::error(
                        "'raise' can only be used inside a guard block",
                        Some(expr.span),
                    ));
                }
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
                IrOperand::Null
            }
            ExprKind::IntLiteral(v) => IrOperand::ConstantInt(*v),
            ExprKind::LongLiteral(v) => IrOperand::ConstantLong(*v),
            ExprKind::DecimalLiteral(v) => IrOperand::ConstantDecimal(*v),
            ExprKind::StringLiteral(v) => {
                if v.contains("${") {
                    let mut parts = Vec::new();
                    let mut curr = v.as_str();
                    while let Some(start) = curr.find("${") {
                        let (prefix, rest) = curr.split_at(start);
                        if !prefix.is_empty() {
                            parts.push(IrOperand::ConstantString(prefix.to_string()));
                        }
                        if let Some(end) = rest.find('}') {
                            let var_name = &rest[2..end];
                            if let Some((ptr, ty)) = self.lookup_var(var_name) {
                                let dest = self.fresh_reg();
                                self.emit(IrInstruction::Load { dest, ptr, ty: ty.clone() });
                                parts.push(IrOperand::Register(dest, ty));
                            }
                            curr = &rest[end + 1..];
                        } else {
                            break;
                        }
                    }
                    if !curr.is_empty() {
                        parts.push(IrOperand::ConstantString(curr.to_string()));
                    }
                    if parts.is_empty() {
                        IrOperand::ConstantString("".into())
                    } else if parts.len() == 1 {
                        parts[0].clone()
                    } else {
                        let mut acc = parts[0].clone();
                        for next in &parts[1..] {
                            let dest = self.fresh_reg();
                            self.emit(IrInstruction::CallRuntime {
                                dest: Some(dest),
                                symbol: "talas_string_concat".into(),
                                args: vec![acc, next.clone()],
                                ret_ty: Type::String,
                            });
                            acc = IrOperand::Register(dest, Type::String);
                        }
                        acc
                    }
                } else {
                    IrOperand::ConstantString(v.clone())
                }
            },
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
                self.build_class_instantiation(class_name, args)
            }
            ExprKind::Call { callee, args } => {
                let arg_ops: Vec<IrOperand> = args.iter().map(|a| self.build_expr(a)).collect();
                match &callee.kind {
                    ExprKind::MemberAccess { object, member, .. } => {
                        let obj_op = self.build_expr(object);
                        let dest = self.fresh_reg();
                        let mut class_name = match obj_op.get_type() {
                            Type::Class { name } => name,
                            _ => "Application".into(),
                        };

                        if class_name == "List" {
                            let symbol = match member.as_str() {
                                "add" => "talas_list_add",
                                "len" => "talas_list_len",
                                _ => "talas_list_len",
                            };
                            let dest = self.fresh_reg();
                            let mut call_args = vec![obj_op];
                            call_args.extend(arg_ops);
                            let ret_ty = if member == "len" { Type::Int } else { Type::Void };
                            self.emit(IrInstruction::CallRuntime {
                                dest: if ret_ty == Type::Void { None } else { Some(dest) },
                                symbol: symbol.into(),
                                args: call_args,
                                ret_ty: ret_ty.clone(),
                            });
                            return if ret_ty == Type::Void { IrOperand::Null } else { IrOperand::Register(dest, ret_ty) };
                        }
                        if class_name == "Map" {
                            let symbol = match member.as_str() {
                                "len" => "talas_map_len",
                                _ => "talas_map_len",
                            };
                            let dest = self.fresh_reg();
                            let mut call_args = vec![obj_op];
                            call_args.extend(arg_ops);
                            let ret_ty = if member == "len" { Type::Int } else { Type::Void };
                            self.emit(IrInstruction::CallRuntime {
                                dest: if ret_ty == Type::Void { None } else { Some(dest) },
                                symbol: symbol.into(),
                                args: call_args,
                                ret_ty: ret_ty.clone(),
                            });
                            return if ret_ty == Type::Void { IrOperand::Null } else { IrOperand::Register(dest, ret_ty) };
                        }
                        if class_name == "Channel" {
                            let symbol = match member.as_str() {
                                "send" => "talas_channel_send",
                                "receive" => "talas_channel_recv",
                                _ => "talas_channel_recv",
                            };
                            let dest = self.fresh_reg();
                            let mut call_args = vec![obj_op];
                            call_args.extend(arg_ops);
                            let ret_ty = if member == "receive" { Type::Int } else { Type::Void };
                            self.emit(IrInstruction::CallRuntime {
                                dest: if ret_ty == Type::Void { None } else { Some(dest) },
                                symbol: symbol.into(),
                                args: call_args,
                                ret_ty: ret_ty.clone(),
                            });
                            return if ret_ty == Type::Void { IrOperand::Null } else { IrOperand::Register(dest, ret_ty) };
                        }
                        if class_name == "Task" || member == "async" {
                            let symbol = match member.as_str() {
                                "async" => "talas_task_create",
                                "all" => "talas_task_all",
                                "any" => "talas_task_any",
                                "race" => "talas_task_race",
                                "allSettled" => "talas_task_all_settled",
                                "cancel" => "talas_task_cancel",
                                _ => "talas_task_await",
                            };
                            let dest = self.fresh_reg();
                            let mut call_args = Vec::new();
                            if member == "cancel" {
                                call_args.push(obj_op);
                            } else {
                                call_args.extend(arg_ops);
                            }
                            let ret_ty = match member.as_str() {
                                "async" => Type::Class { name: "Task".into() },
                                "all" | "allSettled" => Type::Class { name: "List".into() },
                                "cancel" => Type::Bool,
                                _ => Type::Int,
                            };
                            self.emit(IrInstruction::CallRuntime {
                                dest: Some(dest),
                                symbol: symbol.into(),
                                args: call_args,
                                ret_ty: ret_ty.clone(),
                            });
                            return IrOperand::Register(dest, ret_ty);
                        }

                        if class_name == "DateTime" || member == "now" || member == "date" || member == "time" || member == "of" || member == "utc" || member == "parse" || member == "fromUnix" || member == "fromUnixMillis" {
                            let symbol = match member.as_str() {
                                "now" => "talas_datetime_now",
                                "utc" => "talas_datetime_utc",
                                "date" => "talas_datetime_date",
                                "time" => "talas_datetime_time",
                                "of" => "talas_datetime_of",
                                "parse" => "talas_datetime_parse",
                                "fromUnix" => "talas_datetime_from_unix",
                                "fromUnixMillis" => "talas_datetime_from_unix_millis",
                                "year" => "talas_datetime_year",
                                "month" => "talas_datetime_month",
                                "day" => "talas_datetime_day",
                                "hour" => "talas_datetime_hour",
                                "minute" => "talas_datetime_minute",
                                "second" => "talas_datetime_second",
                                "timestamp" => "talas_datetime_timestamp",
                                "unix" => "talas_datetime_unix",
                                "unixMillis" => "talas_datetime_unix_millis",
                                "unixNanos" => "talas_datetime_unix_nanos",
                                "zone" => "talas_datetime_zone",
                                "in" => "talas_datetime_in_zone",
                                "addDays" | "addHours" | "addMinutes" | "addDuration" => "talas_datetime_add_seconds",
                                "diffSeconds" | "diffDays" => "talas_datetime_diff_seconds",
                                "format" => "talas_datetime_format",
                                _ => "talas_datetime_now",
                            };
                            let dest = self.fresh_reg();
                            let mut call_args = Vec::new();
                            if !["now", "utc", "date", "time", "of", "parse", "fromUnix", "fromUnixMillis"].contains(&member.as_str()) {
                                call_args.push(obj_op);
                            }
                            call_args.extend(arg_ops);
                            let ret_ty = match member.as_str() {
                                "zone" | "format" => Type::String,
                                "year" | "month" | "day" | "hour" | "minute" | "second" | "timestamp" | "unix" | "unixMillis" | "unixNanos" | "diffSeconds" | "diffDays" => Type::Int,
                                _ => Type::Class { name: "DateTime".into() },
                            };
                            self.emit(IrInstruction::CallRuntime {
                                dest: Some(dest),
                                symbol: symbol.into(),
                                args: call_args,
                                ret_ty: ret_ty.clone(),
                            });
                            return IrOperand::Register(dest, ret_ty);
                        }

                        if class_name == "Duration" || member == "seconds" || member == "minutes" || member == "hours" || member == "days" {
                            let symbol = match member.as_str() {
                                "seconds" | "minutes" | "hours" | "days" => "talas_duration_create",
                                "totalSeconds" => "talas_duration_total_seconds",
                                _ => "talas_duration_create",
                            };
                            let dest = self.fresh_reg();
                            let mut call_args = Vec::new();
                            if member == "totalSeconds" {
                                call_args.push(obj_op);
                            }
                            call_args.extend(arg_ops);
                            let ret_ty = if member == "totalSeconds" { Type::Int } else { Type::Class { name: "Duration".into() } };
                            self.emit(IrInstruction::CallRuntime {
                                dest: Some(dest),
                                symbol: symbol.into(),
                                args: call_args,
                                ret_ty: ret_ty.clone(),
                            });
                            return IrOperand::Register(dest, ret_ty);
                        }

                        let mut target_method = member.clone();

                        if member == "getHeader" {
                            class_name = "Printable".into();
                            target_method = "getHeader".into();
                        } else if member == "render" && !arg_ops.is_empty() {
                            if let Type::Class { name: arg_cls } = arg_ops[0].get_type() {
                                target_method = format!("{}_{}", member, arg_cls);
                            }
                        }

                        let ret_ty = self
                            .method_return_types
                            .get(&(class_name.clone(), target_method.clone()))
                            .cloned()
                            .or_else(|| {
                                self.method_return_types
                                    .iter()
                                    .find(|((_, m), _)| m == &target_method || m == member)
                                    .map(|(_, ty)| ty.clone())
                            })
                            .unwrap_or(Type::Void);

                        let dest_opt = if ret_ty == Type::Void { None } else { Some(dest) };
                        self.emit(IrInstruction::CallMethod {
                            dest: dest_opt,
                            class_name,
                            method_name: target_method,
                            receiver: obj_op,
                            args: arg_ops,
                            ret_ty: ret_ty.clone(),
                        });
                        if ret_ty == Type::Void {
                            IrOperand::Null
                        } else {
                            IrOperand::Register(dest, ret_ty)
                        }
                    }
                    _ => {
                        self.diagnostics.push(Diagnostic::error(
                            "direct function calls are not supported",
                            Some(expr.span),
                        ));
                        IrOperand::Null
                    }
                }
            }
            ExprKind::MemberAccess { object, member, is_optional } => {
                if *is_optional {
                    self.diagnostics.push(Diagnostic::error(
                        "optional member access '?.' is not yet supported by native code generation",
                        Some(expr.span),
                    ));
                }
                let obj_op = self.build_expr(object);
                let dest = self.fresh_reg();
                let class_name = match obj_op.get_type() {
                    Type::Class { name } => name,
                    _ => "".into(),
                };
                if let Some((idx, field_ty)) = self.class_field_indices.get(&(class_name.clone(), member.clone())).cloned() {
                    if let IrOperand::Register(ptr, _) = obj_op {
                        self.emit(IrInstruction::GetField {
                            dest,
                            ptr,
                            class_name,
                            field_index: idx,
                            field_ty: field_ty.clone(),
                        });
                        IrOperand::Register(dest, field_ty)
                    } else {
                        self.diagnostics.push(Diagnostic::error(
                            format!("cannot lower member access '{}'", member),
                            Some(expr.span),
                        ));
                        IrOperand::Null
                    }
                } else {
                    self.diagnostics.push(Diagnostic::error(
                        format!("cannot lower member access '{}'", member),
                        Some(expr.span),
                    ));
                    IrOperand::Null
                }
            }
            ExprKind::Assign { target, value } => {
                let val_op = self.build_expr(value);
                match &target.kind {
                    ExprKind::MemberAccess { object, member, .. } => {
                        let obj_op = self.build_expr(object);
                        let class_name = match obj_op.get_type() {
                            Type::Class { name } => name,
                            _ => "".into(),
                        };
                        let field_idx_opt = self.class_field_indices.get(&(class_name.clone(), member.clone())).map(|(i, _)| *i);
                        if let Some(idx) = field_idx_opt {
                            if let IrOperand::Register(ptr, _) = obj_op {
                                self.emit(IrInstruction::StoreField {
                                    ptr,
                                    class_name,
                                    field_index: idx,
                                    val: val_op.clone(),
                                });
                            }
                        }
                    }
                    ExprKind::Identifier(name) => {
                        if let Some((ptr, _)) = self.lookup_var(name) {
                            self.emit(IrInstruction::Store {
                                val: val_op.clone(),
                                ptr,
                            });
                        }
                    }
                    _ => {
                        self.diagnostics.push(Diagnostic::error(
                            "left side of assignment is not assignable",
                            Some(expr.span),
                        ));
                    }
                }
                val_op
            }
            ExprKind::Binary { op, left, right } => {
                if op == &BinaryOp::Range {
                    self.diagnostics.push(Diagnostic::error(
                        "range expressions are not yet supported by native code generation",
                        Some(expr.span),
                    ));
                    return IrOperand::ConstantInt(0);
                }
                let l_op = self.build_expr(left);
                let r_op = self.build_expr(right);
                let dest = self.fresh_reg();
                let operand_ty = l_op.get_type();

                let res_ty = match op {
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual
                    | BinaryOp::And
                    | BinaryOp::Nand
                    | BinaryOp::Or
                    | BinaryOp::Nor
                    | BinaryOp::Xor
                    | BinaryOp::Xnor => Type::Bool,
                    _ => operand_ty.clone(),
                };

                self.emit(IrInstruction::Binary {
                    dest,
                    op: op.clone(),
                    left: l_op,
                    right: r_op,
                    operand_ty,
                    result_ty: res_ty.clone(),
                });
                IrOperand::Register(dest, res_ty)
            }
            ExprKind::Unary { op, operand } => {
                let op_val = self.build_expr(operand);
                let dest = self.fresh_reg();
                let ty = op_val.get_type();
                match op {
                    UnaryOp::Not => {
                        self.emit(IrInstruction::Binary {
                            dest,
                            op: BinaryOp::Xor,
                            left: op_val,
                            right: IrOperand::ConstantBool(true),
                            operand_ty: Type::Bool,
                            result_ty: Type::Bool,
                        });
                        IrOperand::Register(dest, Type::Bool)
                    }
                    _ => {
                        self.emit(IrInstruction::Binary {
                            dest,
                            op: BinaryOp::Sub,
                            left: IrOperand::ConstantInt(0),
                            right: op_val,
                            operand_ty: ty.clone(),
                            result_ty: ty.clone(),
                        });
                        IrOperand::Register(dest, ty)
                    }
                }
            }
            ExprKind::ListLiteral(elements) => {
                let list_reg = self.fresh_reg();
                self.emit(IrInstruction::CallRuntime {
                    dest: Some(list_reg),
                    symbol: "talas_list_create".into(),
                    args: vec![],
                    ret_ty: Type::Class { name: "List".into() },
                });
                let list_op = IrOperand::Register(list_reg, Type::Class { name: "List".into() });
                for elem in elements {
                    let elem_op = self.build_expr(elem);
                    self.emit(IrInstruction::CallRuntime {
                        dest: None,
                        symbol: "talas_list_add".into(),
                        args: vec![list_op.clone(), elem_op],
                        ret_ty: Type::Void,
                    });
                }
                list_op
            }
            ExprKind::MapLiteral(pairs) => {
                let map_reg = self.fresh_reg();
                self.emit(IrInstruction::CallRuntime {
                    dest: Some(map_reg),
                    symbol: "talas_map_create".into(),
                    args: vec![],
                    ret_ty: Type::Class { name: "Map".into() },
                });
                let map_op = IrOperand::Register(map_reg, Type::Class { name: "Map".into() });
                for (k, v) in pairs {
                    let k_op = self.build_expr(k);
                    let v_op = self.build_expr(v);
                    self.emit(IrInstruction::CallRuntime {
                        dest: None,
                        symbol: "talas_map_put".into(),
                        args: vec![map_op.clone(), k_op, v_op],
                        ret_ty: Type::Void,
                    });
                }
                map_op
            }
            ExprKind::IndexAccess { target, index } => {
                let target_op = self.build_expr(target);
                let index_op = self.build_expr(index);
                let dest = self.fresh_reg();
                if let Type::Class { ref name } = target_op.get_type() {
                    if name == "Map" {
                        self.emit(IrInstruction::CallRuntime {
                            dest: Some(dest),
                            symbol: "talas_map_get".into(),
                            args: vec![target_op, index_op],
                            ret_ty: Type::Int,
                        });
                        return IrOperand::Register(dest, Type::Int);
                    }
                }
                let len_reg = self.fresh_reg();
                self.emit(IrInstruction::CallRuntime {
                    dest: Some(len_reg),
                    symbol: "talas_list_len".into(),
                    args: vec![target_op.clone()],
                    ret_ty: Type::Int,
                });
                let len_op = IrOperand::Register(len_reg, Type::Int);

                let is_out_of_bounds = self.fresh_reg();
                self.emit(IrInstruction::Binary {
                    dest: is_out_of_bounds,
                    op: BinaryOp::GreaterEqual,
                    left: index_op.clone(),
                    right: len_op,
                    operand_ty: Type::Int,
                    result_ty: Type::Bool,
                });

                let check_target_opt = self.guard_stack.last().cloned();
                if let Some((err_ptr, check_target)) = check_target_opt {
                    let oob_block = self.fresh_block();
                    let ok_block = self.fresh_block();

                    self.emit(IrInstruction::Branch {
                        cond: IrOperand::Register(is_out_of_bounds, Type::Bool),
                        then_block: oob_block,
                        else_block: ok_block,
                    });
                    if let Some(cb) = self.current_block_id {
                        self.finish_block(cb);
                    }

                    self.current_block_id = Some(oob_block);
                    let err_inst = self.build_class_instantiation("OutOfBoundsError", &[Expr {
                        kind: ExprKind::StringLiteral("OutOfBoundsError: List index out of bounds".into()),
                        span: expr.span,
                    }]);
                    self.emit(IrInstruction::Store { val: err_inst, ptr: err_ptr });
                    self.emit(IrInstruction::Jump { target: check_target });
                    self.finish_block(oob_block);

                    self.current_block_id = Some(ok_block);
                }

                self.emit(IrInstruction::CallRuntime {
                    dest: Some(dest),
                    symbol: "talas_list_get".into(),
                    args: vec![target_op, index_op],
                    ret_ty: Type::Int,
                });
                IrOperand::Register(dest, Type::Int)
            }
            ExprKind::AsyncSpawn(inner) => {
                let val_op = self.build_expr(inner);
                let dest = self.fresh_reg();
                self.emit(IrInstruction::CallRuntime {
                    dest: Some(dest),
                    symbol: "talas_task_create".into(),
                    args: vec![val_op],
                    ret_ty: Type::Class { name: "Task".into() },
                });
                IrOperand::Register(dest, Type::Class { name: "Task".into() })
            }
            ExprKind::Await(inner) => {
                let task_op = self.build_expr(inner);
                let dest = self.fresh_reg();
                self.emit(IrInstruction::CallRuntime {
                    dest: Some(dest),
                    symbol: "talas_task_await".into(),
                    args: vec![task_op],
                    ret_ty: Type::Int,
                });
                IrOperand::Register(dest, Type::Int)
            }
            ExprKind::AwaitAndCancel { task, timeout_ms } => {
                let task_op = self.build_expr(task);
                let timeout_op = self.build_expr(timeout_ms);
                let dest = self.fresh_reg();
                self.emit(IrInstruction::CallRuntime {
                    dest: Some(dest),
                    symbol: "talas_task_await_timeout_cancel".into(),
                    args: vec![task_op, timeout_op],
                    ret_ty: Type::Int,
                });
                IrOperand::Register(dest, Type::Int)
            }
            unsupported => {
                self.diagnostics.push(Diagnostic::error(
                    format!("Native code generation for '{:?}' is not implemented yet", unsupported),
                    Some(expr.span),
                ));
                IrOperand::Null
            }
        }
    }
}
