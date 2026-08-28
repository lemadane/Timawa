pub mod diagnostic;
pub mod emitter;

pub use diagnostic::{Diagnostic, DiagnosticLevel, Position, Span};
pub use emitter::DiagnosticEmitter;
