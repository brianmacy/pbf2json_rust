use anyhow::Result;
use clap::{Arg, Command};
use std::path::Path;

mod converter;
mod coordinate_storage;
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
                .default_value("true")
                .help("Enable parallel processing for >800% CPU utilization (enabled by default)"),
        )
        .arg(
            Arg::new("no-parallel")
                .long("no-parallel")
                .action(clap::ArgAction::SetTrue)
                .help("Disable parallel processing and use single-threaded mode"),
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
        .arg(
            Arg::new("temp-db")
                .long("temp-db")
                .value_name("PATH")
                .help("Directory for temporary coordinate database (default: system temp)"),
        )
        .arg(
            Arg::new("keep-temp-db")
                .long("keep-temp-db")
                .action(clap::ArgAction::SetTrue)
                .help("Keep temporary coordinate database after conversion (useful for debugging)"),
        )
        .get_matches();

    let input_path = matches.get_one::<String>("input").unwrap();
    let output_path = matches.get_one::<String>("output");
    let tag_filter = matches.get_one::<String>("tags");
    let pretty_print = matches.get_flag("pretty");
    let use_parallel = !matches.get_flag("no-parallel");
    let geometry_level = matches.get_one::<String>("geometry").unwrap();
    let temp_db_path = matches.get_one::<String>("temp-db");
    let keep_temp_db = matches.get_flag("keep-temp-db");

    if !Path::new(input_path).exists() {
        anyhow::bail!("Input file does not exist: {}", input_path);
    }

    // Parse tag filter supporting both AND (+) and OR (,) logic
    // Format: "tag1+tag2,tag3,tag4+tag5" means (tag1 AND tag2) OR tag3 OR (tag4 AND tag5)
    let tags: Option<Vec<Vec<String>>> = tag_filter.map(|t| {
        t.split(',') // Split by comma for OR groups
            .map(|group| {
                group
                    .split('+') // Split by plus for AND within each group
                    .map(|tag| tag.trim().to_string())
                    .collect::<Vec<String>>()
            })
            .collect::<Vec<Vec<String>>>()
    });

    if use_parallel {
        parallel_converter::convert_pbf_to_geojson_parallel(
            input_path,
            output_path,
            tags,
            pretty_print,
            geometry_level,
            temp_db_path,
            keep_temp_db,
        )?;
    } else {
        converter::convert_pbf_to_geojson_with_geometry_level(
            input_path,
            output_path,
            tags,
            pretty_print,
            geometry_level,
            temp_db_path,
            keep_temp_db,
        )?;
    }

    Ok(())
}
