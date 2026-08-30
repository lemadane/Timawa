use crate::diagnostics::Diagnostic;
use crate::parser::*;
use crate::resolver::{Symbol, SymbolTable};
use crate::types::Type;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct ClassInfo {
    pub name: String,
    pub fields: HashMap<String, (Type, bool)>,
    pub methods: HashMap<String, (Vec<Type>, Type)>,
    pub param_types: Vec<Type>,
    pub min_constructor_args: usize,
    pub max_constructor_args: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraitInfo {
    pub name: String,
    pub methods: HashMap<String, (Vec<Type>, Type)>,
}

pub struct TypeChecker {
    symbol_table: SymbolTable,
    classes: HashMap<String, ClassInfo>,
    traits: HashMap<String, TraitInfo>,
    current_class: Option<String>,
    current_return_type: Option<Type>,
    pub diagnostics: Vec<Diagnostic>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            symbol_table: SymbolTable::new(),
            classes: HashMap::new(),
            traits: HashMap::new(),
            current_class: None,
            current_return_type: None,
            diagnostics: Vec::new(),
        };
        checker.register_builtins();
        checker
    }

    fn register_builtins(&mut self) {
        let mut console_methods = HashMap::new();
        console_methods.insert("print".into(), (vec![Type::String], Type::Void));

        self.classes.insert(
            "Console".into(),
            ClassInfo {
                name: "Console".into(),
                fields: HashMap::new(),
                methods: console_methods,
                param_types: Vec::new(),
                min_constructor_args: 0,
                max_constructor_args: 0,
            },
        );
    }

    pub fn check_program(&mut self, program: &Program) -> bool {
        self.diagnostics.clear();

        for item in &program.items {
            match item {
                Item::Trait(tr) => {
                    let mut methods = HashMap::new();
                    for method in &tr.methods {
                        let param_types = method.params.iter().map(|p| self.type_spec_to_type(&p.type_spec)).collect();
                        let return_ty = method.return_type.as_ref().map_or(Type::Void, |t| self.type_spec_to_type(t));
                        methods.insert(method.name.clone(), (param_types, return_ty));
                    }
                    self.traits.insert(tr.name.clone(), TraitInfo { name: tr.name.clone(), methods });
                }
                Item::Class(cls) => {
                    let mut fields = HashMap::new();
                    let mut methods = HashMap::new();
                    let max_constructor_args = cls.primary_params.len();
                    let min_constructor_args = cls.primary_params.iter().filter(|p| p.default_value.is_none()).count();

                    for param in &cls.primary_params {
                        let field_name = format!("_{}", param.name);
                        let field_ty = self.type_spec_to_type(&param.type_spec);
                        fields.insert(field_name.clone(), (field_ty.clone(), param.is_var));

                        methods.insert(param.name.clone(), (Vec::new(), field_ty));
                    }

                    for case in &cls.cases {
                        let param_types = case.params.iter().map(|p| self.type_spec_to_type(&p.type_spec)).collect();
                        methods.insert(case.name.clone(), (param_types, Type::Class { name: cls.name.clone() }));
                    }

                    for method in &cls.methods {
                        let param_types = method.params.iter().map(|p| self.type_spec_to_type(&p.type_spec)).collect();
                        let return_ty = method.return_type.as_ref().map_or(Type::Void, |t| self.type_spec_to_type(t));
                        methods.insert(method.name.clone(), (param_types, return_ty));
                    }

                    let param_types = cls.primary_params.iter().map(|p| self.type_spec_to_type(&p.type_spec)).collect();

                    self.classes.insert(
                        cls.name.clone(),
                        ClassInfo {
                            name: cls.name.clone(),
                            fields,
                            methods,
                            param_types,
                            min_constructor_args,
                            max_constructor_args,
                        },
                    );
                }
                _ => {}
            }
        }

        for item in &program.items {
            if let Item::Class(cls) = item {
                self.check_class(cls);
            }
        }

        self.diagnostics.is_empty()
    }

    fn check_class(&mut self, cls: &ClassDecl) {
        self.current_class = Some(cls.name.clone());

        for method in &cls.methods {
            self.check_method(cls, method);
        }

        self.current_class = None;
    }

    fn stmt_definitely_returns(&self, stmt: &Stmt) -> bool {
        match &stmt.kind {
            StmtKind::Return(_) | StmtKind::Yield(_) => true,
            StmtKind::If { then_branch, else_branch, .. } => {
                let then_returns = then_branch.iter().any(|s| self.stmt_definitely_returns(s));
                let else_returns = else_branch
                    .as_ref()
                    .map_or(false, |eb| eb.iter().any(|s| self.stmt_definitely_returns(s)));
                then_returns && else_returns
            }
            _ => false,
        }
    }

    fn check_method(&mut self, cls: &ClassDecl, method: &MethodDecl) {
        let return_type = method.return_type.as_ref().map_or(Type::Void, |t| self.type_spec_to_type(t));
        self.current_return_type = Some(return_type.clone());

        if return_type != Type::Void {
            let has_return = method.body.iter().any(|stmt| self.stmt_definitely_returns(stmt));
            if !has_return {
                self.diagnostics.push(Diagnostic::error(
                    format!("Method '{}' must return a value of type '{:?}'", method.name, return_type),
                    Some(method.span),
                ));
            }
        }

        self.symbol_table.push_scope();

        for param in &method.params {
            let p_ty = self.type_spec_to_type(&param.type_spec);
            self.symbol_table.insert(Symbol {
                name: param.name.clone(),
                ty: p_ty,
                is_mutable: false,
            });
        }

        for stmt in &method.body {
            self.check_stmt(cls, stmt);
        }

        self.symbol_table.pop_scope();
        self.current_return_type = None;
    }

    fn check_stmt(&mut self, cls: &ClassDecl, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::LocalDecl { name, is_mutable, type_annotation, initializer } => {
                let init_ty = if let Some(init) = initializer {
                    self.check_expr(cls, init)
                } else {
                    None
                };

                let var_ty = if let Some(ann) = type_annotation {
                    self.type_spec_to_type(ann)
                } else if let Some(ty) = init_ty {
                    ty
                } else {
                    Type::Int
                };

                self.symbol_table.insert(Symbol {
                    name: name.clone(),
                    ty: var_ty,
                    is_mutable: *is_mutable,
                });
            }
            StmtKind::Expr(expr) => {
                self.check_expr(cls, expr);
            }
            StmtKind::If { condition, then_branch, else_branch } => {
                self.check_expr(cls, condition);
                self.symbol_table.push_scope();
                for s in then_branch {
                    self.check_stmt(cls, s);
                }
                self.symbol_table.pop_scope();

                if let Some(eb) = else_branch {
                    self.symbol_table.push_scope();
                    for s in eb {
                        self.check_stmt(cls, s);
                    }
                    self.symbol_table.pop_scope();
                }
            }
            StmtKind::For { loop_var, index_var, condition_or_iterable, body } => {
                self.symbol_table.push_scope();
                if let Some(lvar) = loop_var {
                    self.symbol_table.insert(Symbol {
                        name: lvar.clone(),
                        ty: Type::Int,
                        is_mutable: false,
                    });
                }
                if let Some(ivar) = index_var {
                    self.symbol_table.insert(Symbol {
                        name: ivar.clone(),
                        ty: Type::Int,
                        is_mutable: false,
                    });
                }
                if let Some(cond) = condition_or_iterable {
                    self.check_expr(cls, cond);
                }
                for s in body {
                    self.check_stmt(cls, s);
                }
                self.symbol_table.pop_scope();
            }
            StmtKind::Break => {}
            StmtKind::Continue => {}
            StmtKind::Return(expr_opt) => {
                if let Some(expected_ret) = self.current_return_type.clone() {
                    if expected_ret == Type::Void {
                        if expr_opt.is_some() {
                            self.diagnostics.push(Diagnostic::error(
                                "Cannot return a value from a method returning Void".to_string(),
                                Some(stmt.span),
                            ));
                        }
                    } else {
                        if let Some(e) = expr_opt {
                            let actual_ty = self.check_expr(cls, e);
                            if let Some(act) = actual_ty {
                                if act != expected_ret && act != Type::Void {
                                    self.diagnostics.push(Diagnostic::error(
                                        format!("Mismatched return type: expected '{:?}', found '{:?}'", expected_ret, act),
                                        Some(stmt.span),
                                    ));
                                }
                            }
                        } else {
                            self.diagnostics.push(Diagnostic::error(
                                format!("Non-void method must return a value of type '{:?}'", expected_ret),
                                Some(stmt.span),
                            ));
                        }
                    }
                }
            }
            StmtKind::Yield(expr) => {
                self.check_expr(cls, expr);
            }
        }
    }

    fn check_expr(&mut self, cls: &ClassDecl, expr: &Expr) -> Option<Type> {
        match &expr.kind {
            ExprKind::Print { expr: inner_expr, .. } => {
                self.check_expr(cls, inner_expr);
                Some(Type::Void)
            }
            ExprKind::IntLiteral(_) => Some(Type::Int),
            ExprKind::DecimalLiteral(_) => Some(Type::Decimal),
            ExprKind::StringLiteral(_) => Some(Type::String),
            ExprKind::BoolLiteral(_) => Some(Type::Bool),
            ExprKind::NullLiteral => Some(Type::Nullable(Box::new(Type::Void))),
            ExprKind::This => Some(Type::Class { name: cls.name.clone() }),
            ExprKind::Identifier(name) => {
                let field_name = if name.starts_with('_') { name.clone() } else { format!("_{}", name) };
                if let Some(info) = self.classes.get(&cls.name) {
                    if info.fields.contains_key(&field_name) || info.fields.contains_key(name) {
                        self.diagnostics.push(Diagnostic::error(
                            format!("instance field '{}' must be accessed through 'this'", name),
                            Some(expr.span),
                        ));
                        return None;
                    }
                }

                if let Some(sym) = self.symbol_table.lookup(name) {
                    Some(sym.ty.clone())
                } else {
                    self.diagnostics.push(Diagnostic::error(
                        format!("Undeclared identifier '{}'", name),
                        Some(expr.span),
                    ));
                    None
                }
            }
            ExprKind::MemberAccess { object, member, .. } => {
                let obj_ty = self.check_expr(cls, object)?;
                match obj_ty {
                    Type::Class { name } => {
                        if let Some(info) = self.classes.get(&name) {
                            if let Some((field_ty, _)) = info.fields.get(member) {
                                return Some(field_ty.clone());
                            }
                            if let Some((_, ret_ty)) = info.methods.get(member) {
                                return Some(ret_ty.clone());
                            }
                        }
                        if let Some(tr) = self.traits.get(&name) {
                            if let Some((_, ret_ty)) = tr.methods.get(member) {
                                return Some(ret_ty.clone());
                            }
                        }
                        self.diagnostics.push(Diagnostic::error(
                            format!("Member '{}' not found on target '{}'", member, name),
                            Some(expr.span),
                        ));
                        None
                    }
                    _ => None,
                }
            }
            ExprKind::Call { callee, args } => {
                for arg in args {
                    self.check_expr(cls, arg);
                }
                match &callee.kind {
                    ExprKind::MemberAccess { object, member, .. } => {
                        if let ExprKind::Identifier(class_name) = &object.kind {
                            if let Some(info) = self.classes.get(class_name) {
                                if let Some((_, ret_ty)) = info.methods.get(member) {
                                    return Some(ret_ty.clone());
                                }
                            }
                            if let Some(tr) = self.traits.get(class_name) {
                                if let Some((_, ret_ty)) = tr.methods.get(member) {
                                    return Some(ret_ty.clone());
                                }
                            }
                        }
                        if let ExprKind::This = &object.kind {
                            if let Some(info) = self.classes.get(&cls.name) {
                                if let Some((_, ret_ty)) = info.methods.get(member) {
                                    return Some(ret_ty.clone());
                                }
                            }
                        }
                    }
                    _ => {}
                }
                Some(Type::Void)
            }
            ExprKind::Assign { target, value } => {
                let val_ty = self.check_expr(cls, value)?;
                match &target.kind {
                    ExprKind::MemberAccess { object, member, .. } => {
                        if let ExprKind::This = &object.kind {
                            if let Some(info) = self.classes.get(&cls.name) {
                                let field_info = info.fields.get(member).or_else(|| info.fields.get(&format!("_{}", member)));
                                if let Some((_, is_mutable)) = field_info {
                                    if !is_mutable {
                                        self.diagnostics.push(Diagnostic::error(
                                            format!("cannot reassign immutable field '{}'", member),
                                            Some(target.span),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    ExprKind::Identifier(name) => {
                        if let Some(sym) = self.symbol_table.lookup(name) {
                            if !sym.is_mutable {
                                self.diagnostics.push(Diagnostic::error(
                                    format!("cannot reassign immutable variable '{}'", name),
                                    Some(target.span),
                                ));
                            }
                        }
                    }
                    _ => {}
                }
                Some(val_ty)
            }
            ExprKind::ClassInstantiation { class_name, args } => {
                let mut arg_types = Vec::new();
                for arg in args {
                    if let Some(t) = self.check_expr(cls, arg) {
                        arg_types.push(t);
                    }
                }
                if let Some(info) = self.classes.get(class_name).cloned() {
                    if args.len() > info.max_constructor_args {
                        self.diagnostics.push(Diagnostic::error(
                            format!(
                                "Class '{}' constructor expected at most {} arguments, got {}",
                                class_name,
                                info.max_constructor_args,
                                args.len()
                            ),
                            Some(expr.span),
                        ));
                    } else {
                        for (i, (arg_ty, param_ty)) in arg_types.iter().zip(info.param_types.iter()).enumerate() {
                            if !self.is_assignable_to(arg_ty, param_ty) {
                                self.diagnostics.push(Diagnostic::error(
                                    format!(
                                        "Expected '{:?}' for argument {} of '{}', but got '{:?}'",
                                        param_ty,
                                        i + 1,
                                        class_name,
                                        arg_ty
                                    ),
                                    Some(expr.span),
                                ));
                            }
                        }
                    }
                } else if !self.traits.contains_key(class_name) {
                    self.diagnostics.push(Diagnostic::error(
                        format!("Unknown class '{}'", class_name),
                        Some(expr.span),
                    ));
                }
                Some(Type::Class { name: class_name.clone() })
            }
            ExprKind::Binary { op, left, right } => {
                let l_ty = self.check_expr(cls, left);
                let r_ty = self.check_expr(cls, right);
                match op {
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
                    | BinaryOp::Xnor => Some(Type::Bool),
                    _ => l_ty.or(r_ty).or(Some(Type::Int)),
                }
            }
            ExprKind::Unary { op: _, operand } => {
                let op_ty = self.check_expr(cls, operand);
                op_ty.or(Some(Type::Bool))
            }
            ExprKind::AnonymousObject { fields } => {
                let mut field_types = Vec::new();
                for (name, fexpr) in fields {
                    if let Some(fty) = self.check_expr(cls, fexpr) {
                        field_types.push((name.clone(), fty));
                    }
                }
                Some(Type::AnonymousObject { field_types })
            }
            _ => None,
        }
    }

    fn is_assignable_to(&self, from: &Type, to: &Type) -> bool {
        if from == to || from == &Type::Void {
            return true;
        }
        match (from, to) {
            (Type::Int, Type::Long) | (Type::Long, Type::Int) => true,
            (Type::Int, Type::Decimal) | (Type::Long, Type::Decimal) => true,
            _ => false,
        }
    }

    fn type_spec_to_type(&self, spec: &TypeSpec) -> Type {
        match spec {
            TypeSpec::Int => Type::Int,
            TypeSpec::Decimal => Type::Decimal,
            TypeSpec::Byte => Type::Byte,
            TypeSpec::Long => Type::Long,
            TypeSpec::Bool => Type::Bool,
            TypeSpec::String => Type::String,
            TypeSpec::Class(name) => {
                if name == "void" {
                    Type::Void
                } else {
                    Type::Class { name: name.clone() }
                }
            }
            TypeSpec::Nullable(inner) => Type::Nullable(Box::new(self.type_spec_to_type(inner))),
        }
    }
}
