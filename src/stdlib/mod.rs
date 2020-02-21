use crate::protocol::responses::{
    CompletionItem, CompletionItemKind, InsertTextFormat,
};
use crate::shared::signatures::{get_argument_names, FunctionInfo};
use crate::shared::RequestContext;

use flux::semantic::types::{MonoType, Row};
use libstd::{imports, prelude};

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::iter::Iterator;

use async_trait::async_trait;

fn contains(l: Vec<String>, m: String) -> bool {
    l.into_iter().find(|x| x.as_str() == m.as_str()) != None
}

#[async_trait]
pub trait Completable {
    async fn completion_item(
        &self,
        ctx: RequestContext,
    ) -> CompletionItem;
    fn matches(&self, text: String, imports: Vec<String>) -> bool;
}

#[derive(Clone)]
pub enum VarType {
    Int,
    String,
    Array,
    Float,
    Bool,
    Bytes,
    Duration,
    Regexp,
    Uint,
    Time,
}

#[derive(Clone)]
pub struct VarResult {
    pub name: String,
    pub var_type: VarType,
    pub package: String,
    pub package_name: Option<String>,
}

impl VarResult {
    pub fn detail(&self) -> String {
        match self.var_type {
            VarType::Array => "Array".to_string(),
            VarType::Bool => "Boolean".to_string(),
            VarType::Bytes => "Bytes".to_string(),
            VarType::Duration => "Duration".to_string(),
            VarType::Float => "Float".to_string(),
            VarType::Int => "Integer".to_string(),
            VarType::Regexp => "Regular Expression".to_string(),
            VarType::String => "String".to_string(),
            VarType::Uint => "Uint".to_string(),
            VarType::Time => "Time".to_string(),
        }
    }
}

#[async_trait]
impl Completable for VarResult {
    async fn completion_item(
        &self,
        _ctx: RequestContext,
    ) -> CompletionItem {
        CompletionItem {
            label: format!("{} ({})", self.name, self.package),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: false,
            detail: Some(self.detail()),
            documentation: Some(format!("from {}", self.package)),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.name.clone()),
            insert_text_format: InsertTextFormat::PlainText,
            kind: Some(CompletionItemKind::Variable),
            preselect: None,
            sort_text: Some(format!(
                "{} {}",
                self.name, self.package
            )),
            text_edit: None,
        }
    }

    fn matches(&self, text: String, imports: Vec<String>) -> bool {
        if self.package == "builtin" && !text.ends_with('.') {
            return true;
        }

        if !contains(imports, self.package.clone()) {
            return false;
        }

        if text.ends_with('.') {
            let mtext = text[..text.len() - 1].to_string();
            return Some(mtext) == self.package_name;
        }

        false
    }
}

#[derive(Clone)]
pub struct PackageResult {
    pub name: String,
    pub full_name: String,
}

#[async_trait]
impl Completable for PackageResult {
    async fn completion_item(
        &self,
        _ctx: RequestContext,
    ) -> CompletionItem {
        CompletionItem {
            label: self.name.clone(),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: false,
            detail: Some("Package".to_string()),
            documentation: Some(self.full_name.clone()),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.name.clone()),
            insert_text_format: InsertTextFormat::PlainText,
            kind: Some(CompletionItemKind::Module),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
        }
    }

    fn matches(&self, text: String, imports: Vec<String>) -> bool {
        if !contains(imports, self.full_name.clone()) {
            return false;
        }
        if !text.ends_with('.') {
            let name = self.name.to_lowercase();
            let mtext = text.to_lowercase();
            return name.starts_with(mtext.as_str());
        }

        false
    }
}

#[derive(Clone)]
pub struct FunctionResult {
    pub name: String,
    pub package: String,
    pub package_name: Option<String>,
    pub required_args: Vec<String>,
    pub optional_args: Vec<String>,
    pub signature: String,
}

fn default_arg_insert_text(arg: &str, index: usize) -> String {
    (format!("{}: ${}", arg, index + 1))
}

async fn get_bucket_insert_text(
    arg: &str,
    index: usize,
    ctx: RequestContext,
) -> String {
    if let Ok(buckets) = ctx.callbacks.get_buckets().await {
        if !buckets.is_empty() {
            let list = buckets.join(",");
            let i = format!("${{{}|{}|}}", index + 1, list);

            return format!("{}: ${}", arg, i);
        } else {
            default_arg_insert_text(arg, index)
        }
    } else {
        default_arg_insert_text(arg, index)
    }
}

async fn arg_insert_text(
    arg: &str,
    index: usize,
    ctx: RequestContext,
) -> String {
    match arg {
        "bucket" => get_bucket_insert_text(arg, index, ctx).await,
        _ => default_arg_insert_text(arg, index),
    }
}

impl FunctionResult {
    async fn insert_text(&self, ctx: RequestContext) -> String {
        let mut insert_text = format!("{}(", self.name);

        for (index, arg) in self.required_args.iter().enumerate() {
            insert_text += arg_insert_text(arg, index, ctx.clone())
                .await
                .as_str();

            if index != self.required_args.len() - 1 {
                insert_text += ", ";
            }
        }

        if self.required_args.is_empty()
            && !self.optional_args.is_empty()
        {
            insert_text += "$1";
        }

        insert_text += ")$0";

        insert_text
    }
}

fn make_documentation(package: String) -> String {
    String::from("from ") + package.as_str()
}

#[async_trait]
impl Completable for FunctionResult {
    async fn completion_item(
        &self,
        ctx: RequestContext,
    ) -> CompletionItem {
        CompletionItem {
            label: self.name.clone(),
            additional_text_edits: None,
            commit_characters: None,
            deprecated: false,
            detail: Some(self.signature.clone()),
            documentation: Some(make_documentation(
                self.package.clone(),
            )),
            filter_text: Some(self.name.clone()),
            insert_text: Some(self.insert_text(ctx).await),
            insert_text_format: InsertTextFormat::Snippet,
            kind: Some(CompletionItemKind::Function),
            preselect: None,
            sort_text: Some(self.name.clone()),
            text_edit: None,
        }
    }

    fn matches(&self, text: String, imports: Vec<String>) -> bool {
        if self.package == "builtin" && !text.ends_with('.') {
            return true;
        }

        if !contains(imports, self.package.clone()) {
            return false;
        }

        if text.ends_with('.') {
            let mtext = text[..text.len() - 1].to_string();
            return Some(mtext) == self.package_name;
        }

        false
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub k: String,
    pub v: String,
}

impl fmt::Display for Property {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.k, self.v)
    }
}

struct TVarMap {
    pub mapping: HashMap<flux::semantic::types::Tvar, char>,
    pub current_letter: char,
}

impl TVarMap {
    pub fn default() -> Self {
        TVarMap {
            mapping: HashMap::new(),
            current_letter: 'A',
        }
    }

    fn increment(&mut self) {
        let c = std::char::from_u32(self.current_letter as u32 + 1)
            .unwrap_or(self.current_letter);
        self.current_letter = c
    }

    fn add(&mut self, v: flux::semantic::types::Tvar) -> String {
        let c = self.current_letter;
        self.increment();
        self.mapping.insert(v, c);

        format!("{}", c)
    }

    pub fn get_letter(
        &mut self,
        v: flux::semantic::types::Tvar,
    ) -> String {
        if let Some(result) = self.mapping.get(&v) {
            format!("{}", *result)
        } else {
            self.add(v)
        }
    }
}

fn get_type_string(m: MonoType, map: &mut TVarMap) -> String {
    if let MonoType::Var(t) = m {
        return map.get_letter(t);
    }
    format!("{}", m)
}

pub fn create_function_signature(
    f: flux::semantic::types::Function,
) -> String {
    let mut mapping = TVarMap::default();
    let required = f
        .req
        .iter()
        // Sort args with BTree
        .collect::<BTreeMap<_, _>>()
        .iter()
        .map(|(&k, &v)| Property {
            k: k.clone(),
            v: get_type_string(v.clone(), &mut mapping),
        })
        .collect::<Vec<_>>();

    let optional = f
        .opt
        .iter()
        // Sort args with BTree
        .collect::<BTreeMap<_, _>>()
        .iter()
        .map(|(&k, &v)| Property {
            k: String::from("?") + &k,
            v: get_type_string(v.clone(), &mut mapping),
        })
        .collect::<Vec<_>>();

    let pipe = match f.pipe {
        Some(pipe) => {
            if pipe.k == "<-" {
                vec![Property {
                    k: pipe.k.clone(),
                    v: get_type_string(pipe.v, &mut mapping),
                }]
            } else {
                vec![Property {
                    k: String::from("<-") + &pipe.k,
                    v: get_type_string(pipe.v, &mut mapping),
                }]
            }
        }
        None => vec![],
    };

    format!(
        "({}) -> {}",
        pipe.iter()
            .chain(required.iter().chain(optional.iter()))
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(", "),
        get_type_string(f.retn, &mut mapping)
    )
}

fn walk(
    package: String,
    list: &mut Vec<Box<dyn Completable + Send + Sync>>,
    t: MonoType,
) {
    if let MonoType::Row(row) = t {
        if let Row::Extension { head, tail } = *row {
            match head.v {
                MonoType::Fun(f) => {
                    list.push(Box::new(FunctionResult {
                        name: head.k,
                        package: package.clone(),
                        signature: create_function_signature(
                            (*f).clone(),
                        ),
                        required_args: get_argument_names(f.req),
                        optional_args: get_argument_names(f.opt),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Int => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Int,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Float => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Float,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Bool => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Bool,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Arr(_) => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Array,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Bytes => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Bytes,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Duration => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Duration,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::Regexp => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::Regexp,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                MonoType::String => {
                    list.push(Box::new(VarResult {
                        name: head.k,
                        var_type: VarType::String,
                        package: package.clone(),
                        package_name: get_package_name(
                            package.clone(),
                        ),
                    }));
                }
                _ => {}
            }

            walk(package, list, tail);
        }
    }
}

pub fn get_package_name(name: String) -> Option<String> {
    let items = name.split('/');

    if let Some(n) = items.last() {
        Some(n.to_string())
    } else {
        None
    }
}

pub fn add_package_result(
    name: String,
    list: &mut Vec<Box<dyn Completable + Send + Sync>>,
) {
    let package_name = get_package_name(name.clone());
    if let Some(package_name) = package_name {
        list.push(Box::new(PackageResult {
            name: package_name,
            full_name: name,
        }));
    }
}

fn get_imports(list: &mut Vec<Box<dyn Completable + Send + Sync>>) {
    let env = imports().unwrap();

    for (key, val) in env.values {
        add_package_result(key.clone(), list);
        walk(key, list, val.expr);
    }
}

fn walk_functions(
    package: String,
    list: &mut Vec<FunctionInfo>,
    t: MonoType,
) {
    if let MonoType::Row(row) = t {
        if let Row::Extension { head, tail } = *row {
            if let MonoType::Fun(f) = head.v {
                if let Some(package_name) =
                    get_package_name(package.clone())
                {
                    list.push(FunctionInfo::new(
                        head.k,
                        f.as_ref(),
                        package_name,
                    ));
                }
            }

            walk_functions(package, list, tail);
        }
    }
}

pub fn get_stdlib_functions() -> Vec<FunctionInfo> {
    let mut results = vec![];
    let env = prelude().unwrap();

    for (name, val) in env.values {
        if let MonoType::Fun(f) = val.expr {
            results.push(FunctionInfo::new(
                name,
                f.as_ref(),
                "builtin".to_string(),
            ));
        }
    }

    let impts = imports().unwrap();

    for (name, val) in impts.values {
        walk_functions(name, &mut results, val.expr);
    }

    results
}

pub fn get_builtins(
    list: &mut Vec<Box<dyn Completable + Sync + Send>>,
) {
    let env = prelude().unwrap();

    for (key, val) in env.values {
        match val.expr {
            MonoType::Fun(f) => list.push(Box::new(FunctionResult {
                package: "builtin".to_string(),
                package_name: None,
                name: key.clone(),
                signature: create_function_signature((*f).clone()),
                required_args: get_argument_names(f.req),
                optional_args: get_argument_names(f.opt),
            })),
            MonoType::String => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::String,
            })),
            MonoType::Int => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Int,
            })),
            MonoType::Float => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Float,
            })),
            MonoType::Arr(_) => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Array,
            })),
            MonoType::Bool => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Bool,
            })),
            MonoType::Bytes => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Bytes,
            })),
            MonoType::Duration => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Duration,
            })),
            MonoType::Uint => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Uint,
            })),
            MonoType::Regexp => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Regexp,
            })),
            MonoType::Time => list.push(Box::new(VarResult {
                name: key.clone(),
                package: "builtin".to_string(),
                package_name: None,
                var_type: VarType::Time,
            })),
            _ => {}
        }
    }
}

pub fn get_stdlib() -> Vec<Box<dyn Completable + Sync + Send>> {
    let mut list = vec![];

    get_imports(&mut list);
    get_builtins(&mut list);

    list
}