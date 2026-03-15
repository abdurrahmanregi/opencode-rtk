pub mod compress;
pub mod health;
pub mod optimize;
pub mod shutdown;
pub mod stats;
pub mod tee;

#[cfg(feature = "llm")]
pub mod llm;

use serde_json::Value;

type HandlerResult = Result<Value, (i32, String)>;
