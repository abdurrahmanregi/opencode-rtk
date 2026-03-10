pub mod error_only;
pub mod grouping;
pub mod stats;

use anyhow::Result;

pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;
    fn compress(&self, input: &str) -> Result<String>;
}

pub use error_only::ErrorOnly;
pub use grouping::GroupingByPattern;
pub use stats::StatsExtraction;
