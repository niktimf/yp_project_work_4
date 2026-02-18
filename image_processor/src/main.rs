use std::fs;
use std::path::PathBuf;
use std::process;

use clap::Parser;
use image::GenericImageView as _;
use image_processor::error::AppError;
use image_processor::plugin_loader::PluginLoader;

/// CLI application for processing PNG images
/// using dynamically loaded plugins.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Path to the input PNG image
    #[arg(long)]
    input: PathBuf,

    /// Path to save the processed image
    #[arg(long)]
    output: PathBuf,

    /// Plugin name (without extension, e.g. mirror)
    #[arg(long)]
    plugin: String,

    /// Path to a text file with processing parameters
    #[arg(long)]
    params: PathBuf,

    /// Path to the directory containing plugins
    #[arg(long, default_value = "target/debug")]
    plugin_path: PathBuf,
}

fn run(args: &Args) -> Result<(), AppError> {
    let params =
        fs::read_to_string(&args.params).map_err(|source| AppError::Io {
            path: args.params.clone(),
            source,
        })?;

    log::info!("Loading image: {}", args.input.display());

    let img =
        image::open(&args.input).map_err(|source| AppError::ImageLoad {
            path: args.input.clone(),
            source,
        })?;

    let (width, height) = img.dimensions();
    let mut rgba_image = img.into_rgba8();

    log::info!("Image size: {width}x{height}");

    let rgba_data = rgba_image.as_mut();

    let loader = PluginLoader::load(&args.plugin, &args.plugin_path)?;

    loader.process_image(width, height, rgba_data, &params)?;

    log::info!("Saving result: {}", args.output.display());

    rgba_image
        .save(&args.output)
        .map_err(|source| AppError::ImageSave {
            path: args.output.clone(),
            source,
        })?;

    log::info!("Done!");
    Ok(())
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    if let Err(err) = run(&args) {
        eprintln!("Error: {err}");
        process::exit(1);
    }
}
