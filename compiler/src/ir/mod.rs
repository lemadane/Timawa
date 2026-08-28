pub mod builder;
pub mod nodes;

pub use builder::IrBuilder;
pub use nodes::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::semantics::TypeChecker;

    #[test]
    fn test_ir_building_talas_class() {
        let code = r#"
            class User(long id, string name, var int age = 0) {
                void birthday() {
                    this._age = this._age + 1
                }
            }

            class Application {
                void main() {
                    final user = User(1000, `Lem`, 35)
                    user.birthday()
                }
            }
        "#;
        let lexer = Lexer::new(code);
        let mut parser = Parser::new(lexer);
        let program = parser.parse_program();

        let mut checker = TypeChecker::new();
        checker.check_program(&program);

        let mut builder = IrBuilder::new();
        let ir_program = builder.build_program(&program);

        assert_eq!(ir_program.classes.len(), 2);
        assert!(!ir_program.functions.is_empty());
    }
}
