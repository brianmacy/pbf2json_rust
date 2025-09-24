use anyhow::Result;
use clap::{Arg, Command};
use std::path::Path;

mod converter;
mod osm;
mod parallel_converter;

fn main() -> Result<()> {
    let matches = Command::new("pbf2json")
        .version("0.1.0")
        .about("Convert OpenStreetMap PBF files to GeoJSON")
        .arg(
            Arg::new("input")
                .help("Input PBF file path")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output GeoJSON file (stdout if not specified)"),
        )
        .arg(
            Arg::new("tags")
                .short('t')
                .long("tags")
                .value_name("TAGS")
                .help("Comma-separated list of tags to filter (e.g., highway,building)"),
        )
        .arg(
            Arg::new("pretty")
                .short('p')
                .long("pretty")
                .action(clap::ArgAction::SetTrue)
                .help("Pretty-print JSON output"),
        )
        .arg(
            Arg::new("parallel")
                .long("parallel")
                .action(clap::ArgAction::SetTrue)
                .help("Enable parallel processing for >800% CPU utilization"),
        )
        .arg(
            Arg::new("geometry")
                .short('g')
                .long("geometry")
                .value_name("LEVEL")
                .help("Geometry computation level: auto, basic, full")
                .value_parser(["auto", "basic", "full"])
                .default_value("auto"),
        )
        .get_matches();

    let input_path = matches.get_one::<String>("input").unwrap();
    let output_path = matches.get_one::<String>("output");
    let tag_filter = matches.get_one::<String>("tags");
    let pretty_print = matches.get_flag("pretty");
    let use_parallel = matches.get_flag("parallel");
    let geometry_level = matches.get_one::<String>("geometry").unwrap();

    if !Path::new(input_path).exists() {
        anyhow::bail!("Input file does not exist: {}", input_path);
    }

    let tags: Option<Vec<String>> =
        tag_filter.map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    if use_parallel {
        parallel_converter::convert_pbf_to_geojson_parallel(input_path, output_path, tags, pretty_print)?;
    } else {
        converter::convert_pbf_to_geojson_with_geometry_level(input_path, output_path, tags, pretty_print, geometry_level)?;
    }

    Ok(())
}
