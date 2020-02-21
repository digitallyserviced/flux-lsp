use std::rc::Rc;
use std::sync::Arc;

use crate::handlers::RequestHandler;
use crate::protocol::properties::Position;
use crate::protocol::requests::{
    CompletionParams, PolymorphicRequest, Request,
};
use crate::protocol::responses::{
    CompletionItem, CompletionItemKind, CompletionList,
    InsertTextFormat, Response,
};
use crate::shared::RequestContext;
use crate::stdlib::{get_stdlib, Completable};
use crate::visitors::semantic::{
    utils, CompletableFinderVisitor, ImportFinderVisitor,
    NodeFinderVisitor,
};

use flux::semantic::walk::{self, Node};

use async_trait::async_trait;

fn get_imports(
    uri: String,
    pos: Position,
) -> Result<Vec<String>, String> {
    let pkg = utils::create_completion_package(uri, pos)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = ImportFinderVisitor::default();

    walk::walk(&mut visitor, walker);

    let state = visitor.state.borrow();

    Ok((*state).imports.clone())
}

fn get_ident_name(
    uri: String,
    position: Position,
) -> Result<Option<String>, String> {
    let pkg = utils::create_semantic_package(uri)?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = NodeFinderVisitor::new(position);

    walk::walk(&mut visitor, walker);

    let state = visitor.state.borrow();
    let node = (*state).node.clone();

    if let Some(node) = node {
        match node.as_ref() {
            Node::Identifier(ident) => {
                let name = ident.name.clone();
                return Ok(Some(name));
            }
            Node::IdentifierExpr(ident) => {
                let name = ident.name.clone();
                return Ok(Some(name));
            }
            Node::MemberExpr(mexpr) => {
                if let flux::semantic::nodes::Expression::Identifier(
                    ident,
                ) = &mexpr.object
                {
                    let name = ident.name.clone();
                    return Ok(Some(format!("{}.", name)));
                }
            }
            Node::FunctionParameter(prm) => {
                return Ok(Some(prm.key.clone().name))
            }
            Node::CallExpr(c) => {
                if let Some(arg) = c.arguments.last() {
                    return Ok(Some(arg.key.clone().name));
                }
            }
            _ => {}
        }
    }

    Ok(None)
}

async fn get_stdlib_completions(
    name: String,
    imports: Vec<String>,
    ctx: RequestContext,
) -> Vec<CompletionItem> {
    let mut matches = vec![];
    let completes = get_stdlib();

    for c in completes.into_iter() {
        if c.matches(name.clone(), imports.clone()) {
            matches.push(c.completion_item(ctx.clone()).await);
        }
    }

    matches
}

fn get_user_completables(
    uri: String,
    pos: Position,
) -> Result<Vec<Arc<dyn Completable + Send + Sync>>, String> {
    let pkg = utils::create_completion_package(uri, pos.clone())?;
    let walker = Rc::new(walk::Node::Package(&pkg));
    let mut visitor = CompletableFinderVisitor::new(pos);

    walk::walk(&mut visitor, walker);

    if let Ok(state) = visitor.state.lock() {
        return Ok((*state).completables.clone());
    }

    Err("failed to get completables".to_string())
}

async fn get_user_matches(
    uri: String,
    name: String,
    imports: Vec<String>,
    pos: Position,
    ctx: RequestContext,
) -> Result<Vec<CompletionItem>, String> {
    let completables =
        get_user_completables(uri.clone(), pos.clone())?;

    let filtered: Vec<Arc<dyn Completable + Send + Sync>> =
        completables
            .clone()
            .into_iter()
            .filter(|x| x.matches(name.clone(), imports.clone()))
            .collect();

    let mut result: Vec<CompletionItem> = vec![];
    for x in filtered {
        result.push(x.completion_item(ctx.clone()).await)
    }

    Ok(result)
}

async fn find_completions(
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<CompletionList, String> {
    let uri = params.text_document.uri;
    let pos = params.position.clone();
    let name = get_ident_name(uri.clone(), params.position)?;

    let mut items: Vec<CompletionItem> = vec![];
    let imports = get_imports(uri.clone(), pos.clone())?;

    if let Some(name) = name {
        let mut stdlib_matches = get_stdlib_completions(
            name.clone(),
            imports.clone(),
            ctx.clone(),
        )
        .await;
        items.append(&mut stdlib_matches);

        let mut user_matches =
            get_user_matches(uri, name, imports, pos, ctx).await?;

        items.append(&mut user_matches);
    }

    Ok(CompletionList {
        is_incomplete: false,
        items,
    })
}

fn new_arg_completion(value: String) -> CompletionItem {
    CompletionItem {
        deprecated: false,
        commit_characters: None,
        detail: None,
        label: value,
        additional_text_edits: None,
        filter_text: None,
        insert_text: None,
        documentation: None,
        sort_text: None,
        preselect: None,
        insert_text_format: InsertTextFormat::PlainText,
        text_edit: None,
        kind: Some(CompletionItemKind::Text),
    }
}

async fn find_arg_completions(
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<CompletionList, String> {
    let uri = params.text_document.uri;
    let name = get_ident_name(uri, params.position)?;

    if let Some(name) = name {
        if name == "bucket" {
            let buckets = ctx.callbacks.get_buckets().await?;

            let items: Vec<CompletionItem> =
                buckets.into_iter().map(new_arg_completion).collect();

            return Ok(CompletionList {
                is_incomplete: false,
                items,
            });
        }
    }

    Ok(CompletionList {
        is_incomplete: false,
        items: vec![],
    })
}

async fn all_completions(
    params: CompletionParams,
    ctx: RequestContext,
) -> Result<CompletionList, String> {
    if let Some(context) = params.clone().context {
        if let Some(trigger) = context.trigger_character {
            if trigger == ":" {
                return find_arg_completions(params, ctx).await;
            }
        }
    }

    find_completions(params, ctx).await
}

#[derive(Default)]
pub struct CompletionHandler {}

#[async_trait]
impl RequestHandler for CompletionHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        ctx: RequestContext,
    ) -> Result<Option<String>, String> {
        let req: Request<CompletionParams> =
            Request::from_json(prequest.data.as_str())?;
        if let Some(params) = req.params {
            let completions = all_completions(params, ctx).await?;

            let response = Response::new(
                prequest.base_request.id,
                Some(completions),
            );

            let result = response.to_json()?;

            return Ok(Some(result));
        }

        Err("invalid completion request".to_string())
    }
}