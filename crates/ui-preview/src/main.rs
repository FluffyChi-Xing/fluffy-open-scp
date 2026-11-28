use std::{error::Error, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.as_slice() {
        [command, directory] if command == "init" => ui_preview::init(Path::new(directory))?,
        [command, directory, picture] if command == "publish" => {
            println!(
                "{}",
                ui_preview::publish(Path::new(directory), Path::new(picture))?
            );
        }
        [command, directory, port @ ..] if command == "serve" && port.len() <= 1 => {
            let port = port
                .first()
                .map(|p| p.to_string_lossy().parse())
                .transpose()?
                .unwrap_or(ui_preview::DEFAULT_PORT);
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(ui_preview::serve(Path::new(directory), port))?;
        }
        _ => {
            return Err(
                "usage: ui-preview init <dir> | publish <dir> <png> | serve <dir> [port]".into(),
            );
        }
    }
    Ok(())
}
