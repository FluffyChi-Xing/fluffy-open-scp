use sc_exporter::{TextureOutputFormat, export_texture};
use std::path::PathBuf;
use std::process::ExitCode;

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let input =
        PathBuf::from(args.next().ok_or(
            "Usage: export-texture <file.rw4> <section> <output> [--format png|jpg|tga|dds]",
        )?);
    let section: u32 = args
        .next()
        .ok_or("missing section")?
        .parse()
        .map_err(|_| "invalid section")?;
    let output = PathBuf::from(args.next().ok_or("missing output")?);
    let mut format = output
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("png")
        .parse::<TextureOutputFormat>()
        .map_err(|e| e.to_string())?;
    while let Some(arg) = args.next() {
        if arg == "--format" {
            format = args
                .next()
                .ok_or("missing format")?
                .parse()
                .map_err(|e: String| e)?;
        } else {
            return Err(format!("unknown argument: {arg}"));
        }
    }
    let data = std::fs::read(&input).map_err(|e| e.to_string())?;
    let file = rw4::Rw4File::parse(&data).map_err(|e| e.to_string())?;
    let texture = file
        .decode_texture(&data, section)
        .map_err(|e| e.to_string())?;
    std::fs::write(
        &output,
        export_texture(&texture, format).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ERROR: {e}");
            ExitCode::FAILURE
        }
    }
}
