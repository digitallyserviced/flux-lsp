pub mod document_change;
pub mod document_open;
pub mod goto_definition;
pub mod initialize;
pub mod references;
pub mod rename;
pub mod shutdown;

use crate::structs::{
    create_diagnostics_notification, Notification,
    PolymorphicRequest, Position, PublishDiagnosticsParams,
};
use crate::utils;
use crate::visitors::NodeFinderVisitor;

use std::rc::Rc;

use flux::ast::{check, walk};

pub trait RequestHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<String, String>;
}

pub fn create_file_diagnostics(
    uri: String,
) -> Result<Notification<PublishDiagnosticsParams>, String> {
    let file = utils::create_file_node(uri.clone())?;
    let walker = walk::Node::File(&file);

    let errors = check::check(walker);
    let diagnostics = utils::map_errors_to_diagnostics(errors);

    match create_diagnostics_notification(uri.clone(), diagnostics) {
        Ok(msg) => Ok(msg),
        Err(e) => Err(format!("Failed to create diagnostic: {}", e)),
    }
}

#[derive(Default, Clone)]
pub struct NodeFinderResult<'a> {
    node: Option<Rc<walk::Node<'a>>>,
    path: Vec<Rc<walk::Node<'a>>>,
}

pub fn find_node<'a>(
    file: &'a flux::ast::File,
    position: Position,
) -> NodeFinderResult<'a> {
    let mut result = NodeFinderResult::default();
    let walker: walk::Node<'a> = walk::Node::File(file);
    let visitor = NodeFinderVisitor::new(position);

    walk::walk(&visitor, walker);

    let state = visitor.state.borrow();

    result.node = (*state).node.clone();
    result.path = (*state).path.clone();

    result
}
