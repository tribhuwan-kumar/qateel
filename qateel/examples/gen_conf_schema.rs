use std::fs;
use std::path::Path;
use qateel::config::Config;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new("schema/config.schema.json");
	let schema = schemars::schema_for!(Config);
	let value = serde_json::to_value(&schema)?;

	if let Ok(existing) = fs::read_to_string(path) {
		if let Ok(existing_value) = serde_json::from_str::<serde_json::Value>(&existing) {
			if existing_value == value {
				println!("SUCCESS: Schema file is unchanged: '{}'", path.display());
				return Ok(());
			}
		}
	}
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent)?;
	}

	let string_pretty = serde_json::to_string_pretty(&schema)?;
	fs::write(path, string_pretty)?;
    println!("SUCCESS: Updated schema file at '{}'", path.display());

	Ok(())
}
