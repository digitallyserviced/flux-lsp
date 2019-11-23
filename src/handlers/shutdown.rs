use crate::handlers::RequestHandler;

use crate::protocol::requests::PolymorphicRequest;
use crate::protocol::responses::{Response, ShutdownResult};

pub struct ShutdownHandler {}

impl RequestHandler for ShutdownHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        let id = prequest.base_request.id;
        let response: Response<ShutdownResult> =
            Response::new(id, None);

        let json = response.to_json()?;
        Ok(Some(json))
    }
}

impl Default for ShutdownHandler {
    fn default() -> Self {
        ShutdownHandler {}
    }
}