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
                        self.method_return_types.insert((tr.name.clone(), method.name.clone()), ret_ty);
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
        self.emit(IrInstruction::Return { val: None });
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
                let cond_op = self.build_expr(condition);
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
                            .unwrap_or_else(|| {
                                if member.starts_with("get") || member == "getHeader" || member == "name" {
                                    Type::String
                                } else {
                                    Type::Void
                                }
                            });

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
                    _ => IrOperand::Null,
                }
            }
            ExprKind::MemberAccess { object, member, .. } => {
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
                        IrOperand::ConstantInt(0)
                    }
                } else {
                    IrOperand::ConstantInt(0)
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
                    _ => {}
                }
                val_op
            }
            ExprKind::Binary { op, left, right } => {
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
