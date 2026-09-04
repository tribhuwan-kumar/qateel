use file_format::{FileFormat, Kind};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let formats = FileFormat::from_extension("svg");
	let kind = Kind::Database();

	println!("FORMATS: {:?}", formats);
    Ok(())
}

