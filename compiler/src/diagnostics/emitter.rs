use super::diagnostic::{Diagnostic, DiagnosticLevel};

pub struct DiagnosticEmitter;

impl DiagnosticEmitter {
    pub fn emit(diagnostics: &[Diagnostic], source_code: &str) {
        for diag in diagnostics {
            let level_str = match diag.level {
                DiagnosticLevel::Error => "error",
                DiagnosticLevel::Warning => "warning",
                DiagnosticLevel::Note => "note",
            };

            if let Some(span) = diag.span {
                eprintln!(
                    "[{}] at line {}, col {}: {}",
                    level_str, span.start.line, span.start.column, diag.message
                );
                let lines: Vec<&str> = source_code.lines().collect();
                if span.start.line > 0 && span.start.line <= lines.len() {
                    let line_content = lines[span.start.line - 1];
                    eprintln!("  | {}", line_content);
                    let padding = " ".repeat(span.start.column.saturating_sub(1));
                    let underline_len = if span.end.line == span.start.line {
                        (span.end.column.saturating_sub(span.start.column)).max(1)
                    } else {
                        1
                    };
                    let underline = "^".repeat(underline_len);
                    eprintln!("  | {}{}", padding, underline);
                }
            } else {
                eprintln!("[{}]: {}", level_str, diag.message);
            }
        }
    }
}
