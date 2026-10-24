use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProbeResult {
    pub edition: String,
    pub slots: [String; 3],
}
