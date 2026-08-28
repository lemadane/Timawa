use crate::diagnostics::{Diagnostic, Span};
use crate::parser::*;
use crate::resolver::{Symbol, SymbolTable};
use crate::types::Type;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct ClassInfo {
    pub name: String,
    pub fields: HashMap<String, (Type, bool)>,
    pub methods: HashMap<String, (Vec<Type>, Type)>,
}

pub struct TypeChecker {
    symbol_table: SymbolTable,
    classes: HashMap<String, ClassInfo>,
    current_class: Option<String>,
    current_return_type: Option<Type>,
    pub diagnostics: Vec<Diagnostic>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            symbol_table: SymbolTable::new(),
            classes: HashMap::new(),
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
            },
        );
    }

    pub fn check_program(&mut self, program: &Program) -> bool {
        for item in &program.items {
            if let Item::Class(cls) = item {
                let mut fields = HashMap::new();
                let mut methods = HashMap::new();

                for param in &cls.primary_params {
                    let field_name = format!("_{}", param.name);
                    let field_ty = self.type_spec_to_type(&param.type_spec);
                    fields.insert(field_name.clone(), (field_ty.clone(), param.is_var));

                    methods.insert(param.name.clone(), (Vec::new(), field_ty));
                }

                for method in &cls.methods {
                    let param_types = method.params.iter().map(|p| self.type_spec_to_type(&p.type_spec)).collect();
                    let return_ty = method.return_type.as_ref().map_or(Type::Void, |t| self.type_spec_to_type(t));
                    methods.insert(method.name.clone(), (param_types, return_ty));
                }

                self.classes.insert(
                    cls.name.clone(),
                    ClassInfo {
                        name: cls.name.clone(),
                        fields,
                        methods,
                    },
                );
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

    fn check_method(&mut self, cls: &ClassDecl, method: &MethodDecl) {
        let return_type = method.return_type.as_ref().map_or(Type::Void, |t| self.type_spec_to_type(t));
        self.current_return_type = Some(return_type);

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
            StmtKind::For { condition_or_iterable, body, .. } => {
                if let Some(cond) = condition_or_iterable {
                    self.check_expr(cls, cond);
                }
                self.symbol_table.push_scope();
                for s in body {
                    self.check_stmt(cls, s);
                }
                self.symbol_table.pop_scope();
            }
            StmtKind::Return(expr_opt) => {
                if let Some(e) = expr_opt {
                    self.check_expr(cls, e);
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
                        self.diagnostics.push(Diagnostic::error(
                            format!("Member '{}' not found on class '{}'", member, name),
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
                                if let Some((_, is_mutable)) = info.fields.get(member) {
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
                val_ty
            }
            ExprKind::ClassInstantiation { class_name, args } => {
                for arg in args {
                    self.check_expr(cls, arg);
                }
                Some(Type::Class { name: class_name.clone() })
            }
            ExprKind::Binary { op: _, left, right } => {
                self.check_expr(cls, left);
                self.check_expr(cls, right);
                Some(Type::Bool)
            }
            ExprKind::Unary { operand, .. } => {
                self.check_expr(cls, operand);
                Some(Type::Bool)
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

    fn type_spec_to_type(&self, spec: &TypeSpec) -> Type {
        match spec {
            TypeSpec::Int => Type::Int,
            TypeSpec::Decimal => Type::Decimal,
            TypeSpec::Byte => Type::Byte,
            TypeSpec::Long => Type::Long,
            TypeSpec::Bool => Type::Bool,
            TypeSpec::String => Type::String,
            TypeSpec::Class(name) => Type::Class { name: name.clone() },
            TypeSpec::Nullable(inner) => Type::Nullable(Box::new(self.type_spec_to_type(inner))),
        }
    }
}
