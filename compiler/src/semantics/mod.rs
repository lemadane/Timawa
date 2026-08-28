pub mod type_checker;

pub use type_checker::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    #[test]
    fn test_explicit_this_enforcement() {
        let code = r#"
            class User(long id, var int age) {
                void birthday() {
                    _age = _age + 1
                }
            }
        "#;
        let lexer = Lexer::new(code);
        let mut parser = Parser::new(lexer);
        let program = parser.parse_program();

        let mut checker = TypeChecker::new();
        checker.check_program(&program);

        assert!(!checker.diagnostics.is_empty());
        let err_msg = &checker.diagnostics[0].message;
        assert!(err_msg.contains("must be accessed through 'this'"));
    }

    #[test]
    fn test_immutable_field_protection() {
        let code = r#"
            class User(long id, string name) {
                void updateName(string newName) {
                    this._name = newName
                }
            }
        "#;
        let lexer = Lexer::new(code);
        let mut parser = Parser::new(lexer);
        let program = parser.parse_program();

        let mut checker = TypeChecker::new();
        checker.check_program(&program);

        assert!(!checker.diagnostics.is_empty());
        let err_msg = &checker.diagnostics[0].message;
        assert!(err_msg.contains("cannot reassign immutable field"));
    }
}
