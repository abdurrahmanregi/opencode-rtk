pub mod compress;
pub mod health;
pub mod stats;
pub mod shutdown;

use serde_json::Value;

type HandlerResult = Result<Value, (i32, String)>;
