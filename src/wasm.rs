/* There are many `allow(dead_code)` pragmas in this file. The reason
 * these are necessary is because they are used solely by the wasm build
 * process, and aren't used when building the lib itself. We want still want
 * the "X is never used" messages in this file, so we mark only the things we
 * know are being used with the pragma. There is an integration test that
 * we can use to assert what is actually being used here.
 */
use crate::handlers::{Error, Router};
use crate::shared::callbacks::Callbacks;
use crate::shared::messages::{
    create_polymorphic_request, wrap_message,
};
use crate::shared::RequestContext;

use std::cell::RefCell;
use std::ops::Add;
use std::rc::Rc;

use js_sys::{Function, Promise};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

#[wasm_bindgen]
pub struct Server {
    handler: Rc<RefCell<Router>>,
    callbacks: Callbacks,
    support_multiple_files: bool,
}

#[wasm_bindgen]
#[derive(Deserialize)]
struct ServerResponse {
    #[allow(dead_code)]
    message: Option<String>,
    #[allow(dead_code)]
    error: Option<String>,
}

#[derive(Serialize)]
struct ServerError {
    id: u32,
    error: ResponseError,
    jsonrpc: String,
}

impl ServerError {
    fn from_error(id: u32, err: Error) -> Result<String, Error> {
        let se = ServerError {
            id,
            error: ResponseError {
                code: 100,
                message: err.msg,
            },
            jsonrpc: "2.0".to_string(),
        };

        match serde_json::to_string(&se) {
            Ok(val) => Ok(val),
            Err(_) => Err(Error {
                msg: "failed to serialize error".to_string(),
            }),
        }
    }
}

#[derive(Serialize)]
struct ResponseError {
    code: u32,
    message: String,
}

#[wasm_bindgen]
impl ServerResponse {
    #[allow(dead_code)]
    pub fn get_message(&self) -> Option<String> {
        self.message.clone()
    }

    #[allow(dead_code)]
    pub fn get_error(&self) -> Option<String> {
        self.error.clone()
    }
}

#[wasm_bindgen]
impl Server {
    #[wasm_bindgen(constructor)]
    pub fn new(
        disable_folding: bool,
        support_multiple_files: bool,
    ) -> Server {
        Server {
            handler: Rc::new(RefCell::new(Router::new(
                disable_folding,
            ))),
            callbacks: Callbacks::default(),
            support_multiple_files,
        }
    }

    pub fn register_buckets_callback(&mut self, f: Function) {
        self.callbacks.register_buckets_callback(f);
    }

    pub fn register_measurements_callback(&mut self, f: Function) {
        self.callbacks.register_measurements_callback(f);
    }

    pub fn register_tag_keys_callback(&mut self, f: Function) {
        self.callbacks.register_tag_keys_callback(f);
    }

    pub fn register_tag_values_callback(&mut self, f: Function) {
        self.callbacks.register_tag_values_callback(f);
    }

    pub fn process(&mut self, msg: String) -> Promise {
        let router = self.handler.clone();
        let callbacks = self.callbacks.clone();
        let support_multiple_files = self.support_multiple_files;

        future_to_promise(async move {
            let lines = msg.lines();
            let content: String =
                lines.skip(2).fold(String::new(), |c, l| c.add(l));

            match create_polymorphic_request(content.clone()) {
                Ok(req) => {
                    let id = req.base_request.id;
                    let ctx = RequestContext::new(
                        callbacks.clone(),
                        support_multiple_files,
                    );
                    let mut h = router.borrow_mut();
                    match (*h).route(req, ctx).await {
                        Ok(response) => {
                            if let Some(response) = response {
                                Ok(JsValue::from(ServerResponse {
                                    message: Some(wrap_message(
                                        response,
                                    )),
                                    error: None,
                                }))
                            } else {
                                Ok(JsValue::from(ServerResponse {
                                    message: None,
                                    error: None,
                                }))
                            }
                        }
                        Err(error) => {
                            Ok(JsValue::from(ServerResponse {
                                message: Some(wrap_message(
                                    ServerError::from_error(
                                        id, error,
                                    )
                                    .unwrap(),
                                )),
                                error: None,
                            }))
                        }
                    }
                }
                Err(e) => Ok(JsValue::from(ServerResponse {
                    message: None,
                    error: Some(format!("{} -> {}", e, content)),
                })),
            }
        })
    }
}
