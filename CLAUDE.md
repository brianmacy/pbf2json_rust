# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust implementation of pbf2json - a tool for converting OpenStreetMap PBF (Protocol Buffer Format) files to GeoJSON format. The project aims to provide a fast, memory-efficient alternative to existing pbf2json tools, leveraging Rust's performance characteristics.

## Development Commands

### Initial Setup
- `cargo init` - Initialize new Rust project (if not done)
- `cargo add <crate>` - Add dependencies to Cargo.toml

### Building and Testing
- `cargo build` - Build the project
- `cargo build --release` - Build optimized release version
- `cargo test` - Run all tests
- `cargo test <test_name>` - Run specific test
- `cargo run` - Run the application
- `cargo run -- <args>` - Run with command line arguments

### Code Quality
- `cargo fmt` - Format code according to Rust standards
- `cargo clippy` - Run Clippy linter with warnings as errors
- `cargo clippy --all-targets --all-features -- -D warnings` - Comprehensive linting

## Architecture Guidelines

### PBF to GeoJSON Conversion Flow
The tool should follow this general architecture:
1. **PBF Parser**: Read and decode Protocol Buffer format OSM data
2. **Data Model**: Internal representation of OSM entities (nodes, ways, relations)
3. **Geometry Builder**: Construct geometries from OSM data relationships
4. **GeoJSON Serializer**: Convert geometries to GeoJSON format
5. **Stream Processing**: Handle large files efficiently with streaming

### Key Components to Implement
- **OSM Data Types**: Nodes (points), Ways (lines/polygons), Relations (complex geometries)
- **Coordinate Resolution**: Convert OSM lat/lon coordinates to GeoJSON format
- **Tag Filtering**: Allow selective export based on OSM tags
- **Memory Management**: Stream processing for large PBF files (multi-GB)
- **Error Handling**: Robust error handling for malformed PBF data

### Dependencies Considerations
Based on typical Rust PBF/GeoJSON tools, likely dependencies include:
- `protobuf` or `prost` - Protocol Buffer support
- `osmpbf` - OpenStreetMap PBF format parsing
- `geojson` - GeoJSON format support
- `serde` - Serialization/deserialization
- `clap` - Command line argument parsing
- `anyhow` or `thiserror` - Error handling

### Performance Requirements
- Handle large OSM extracts (1GB+ PBF files)
- Memory-efficient streaming processing
- Multi-threaded processing where appropriate
- Configurable output formatting (pretty-print vs compact)

### CLI Interface
The tool should provide a command-line interface similar to:
```
pbf2json input.osm.pbf > output.geojson
pbf2json --tags highway,building input.osm.pbf > filtered.geojson
```

### Reference Implementation
The existing pelias/pbf2json (written in C++) serves as the reference implementation for functionality and CLI interface compatibility.
- Keep a close eye on how GoLang pbf2json parallelizes with a flat memory footprint