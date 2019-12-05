use crate::protocol::properties::{Diagnostic, Position, Range};
use crate::protocol::requests::{BaseRequest, PolymorphicRequest};

use flux::ast::{self, check};
use flux::parser::parse_string;

pub fn wrap_message(s: String) -> String {
    let st = s.clone();
    let result = st.as_bytes();
    let size = result.len();

    format!("Content-Length: {}\r\n\r\n{}", size, s)
}

pub fn get_content_size(s: String) -> Result<usize, String> {
    let tmp = String::from(s.trim_end());
    let stmp: Vec<&str> = tmp.split(": ").collect();

    match String::from(stmp[1]).parse::<usize>() {
        Ok(size) => Ok(size),
        Err(_) => Err("Failed to parse content size".to_string()),
    }
}

pub fn parse_request(
    content: String,
) -> Result<PolymorphicRequest, String> {
    let request = BaseRequest::from_json(content.as_str())?;

    let result = PolymorphicRequest {
        base_request: request,
        data: content.clone(),
    };

    Ok(result)
}

pub fn map_errors_to_diagnostics(
    errors: Vec<check::Error>,
) -> Vec<Diagnostic> {
    let mut result = vec![];

    for error in errors {
        result.push(map_error_to_diagnostic(error));
    }

    result
}

pub fn create_file_node_from_text(
    uri: String,
    text: String,
) -> ast::File {
    parse_string(uri.as_str(), text.as_str())
}

// TODO: figure out if all clients are zero based or if its
//       just vim-lsp if not remove the hard coded
//       subtraction in favor of runtime options
fn map_error_to_diagnostic(error: check::Error) -> Diagnostic {
    Diagnostic {
        severity: 1,
        code: 1,
        message: error.message,
        range: Range {
            start: Position {
                line: error.location.start.line - 1,
                character: error.location.start.column - 1,
            },
            end: Position {
                line: error.location.end.line - 1,
                character: error.location.end.column - 1,
            },
        },
    }
}
