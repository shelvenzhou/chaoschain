use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentInfo {
    pub name: String,
    pub system: String,
}

#[derive(Debug, Deserialize)]
struct InputJson {
    name: String,
    system: String,
    /// other fields are ignored
    #[serde(flatten)]
    _other: serde_json::Value,
}

pub fn read_agent_info<P: AsRef<Path>>(
    file_path: P,
) -> Result<AgentInfo, Box<dyn std::error::Error>> {
    let file_content =
        fs::read_to_string(file_path).map_err(|e| format!("Failed to read file: {}", e))?;

    let input: InputJson =
        serde_json::from_str(&file_content).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    Ok(AgentInfo {
        name: input.name,
        system: input.system,
    })
}
