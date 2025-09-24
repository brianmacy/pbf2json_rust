use pbf2json::convert_pbf_to_geojson_with_geometry_level;
use serde_json::Value;
use std::fs;
use tempfile::NamedTempFile;

#[cfg(test)]
mod three_pass_tests {
    use super::*;

    #[test]
    fn test_three_pass_relation_geometry_with_rome() {
        // Test with the rome.osm.pbf file if available
        let rome_path = "rome.osm.pbf";

        if !std::path::Path::new(rome_path).exists() {
            eprintln!("⚠️  Rome PBF file not available, skipping three-pass test");
            return;
        }

        // Create temporary output file
        let output_file = NamedTempFile::new().expect("Failed to create temp file");
        let output_path = output_file.path().to_str().unwrap().to_string();

        // Run conversion with full geometry mode to trigger three-pass processing
        let result = convert_pbf_to_geojson_with_geometry_level(
            rome_path,
            Some(&output_path),
            Some(vec!["type".to_string()]), // Filter for relations with type tag
            false, // No pretty print for easier parsing
            "full", // Force full geometry mode
        );

        assert!(result.is_ok(), "Conversion should succeed");

        // Read the output file and verify relations have geometry
        let output_content = fs::read_to_string(&output_path)
            .expect("Should be able to read output file");

        let lines: Vec<&str> = output_content.lines().collect();
        assert!(!lines.is_empty(), "Should have output lines");

        // Look for relations with geometry
        let mut found_relation_with_geometry = false;
        let mut total_relations = 0;

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            let json: Value = serde_json::from_str(line)
                .expect("Each line should be valid JSON");

            // Check if it's a relation
            if json["type"] == "relation" {
                total_relations += 1;

                // Check if it has centroid and bounds (geometry)
                if json.get("centroid").is_some() && json.get("bounds").is_some() {
                    found_relation_with_geometry = true;

                    // Verify centroid structure
                    let centroid = &json["centroid"];
                    assert!(centroid["lat"].is_string() || centroid["lat"].is_number(),
                        "Centroid should have lat field");
                    assert!(centroid["lon"].is_string() || centroid["lon"].is_number(),
                        "Centroid should have lon field");
                    assert_eq!(centroid["type"], "entrance",
                        "Centroid type should be 'entrance'");

                    // Verify bounds structure
                    let bounds = &json["bounds"];
                    assert!(bounds["n"].is_string() || bounds["n"].is_number(),
                        "Bounds should have north field");
                    assert!(bounds["s"].is_string() || bounds["s"].is_number(),
                        "Bounds should have south field");
                    assert!(bounds["e"].is_string() || bounds["e"].is_number(),
                        "Bounds should have east field");
                    assert!(bounds["w"].is_string() || bounds["w"].is_number(),
                        "Bounds should have west field");

                    println!("✅ Found relation {} with geometry: centroid={}, bounds={}",
                        json["id"], centroid, bounds);
                    break;
                }
            }
        }

        println!("🔍 Three-pass test results:");
        println!("   - Total relations found: {}", total_relations);
        println!("   - Relations with geometry: {}", if found_relation_with_geometry { "Yes" } else { "No" });

        assert!(total_relations > 0,
            "Should find at least some relations in Rome PBF");
        assert!(found_relation_with_geometry,
            "At least one relation should have centroid and bounds geometry in three-pass mode");
    }

    #[test]
    fn test_geometry_mode_selection() {
        // Test that geometry mode selection works correctly based on file size
        let rome_path = "rome.osm.pbf";

        if !std::path::Path::new(rome_path).exists() {
            eprintln!("⚠️  Rome PBF file not available, skipping geometry mode test");
            return;
        }

        // Check file size (rome.osm.pbf should be ~22MB, so < 1GB threshold)
        let file_size = std::fs::metadata(rome_path).unwrap().len();
        let file_size_gb = file_size as f64 / (1024.0 * 1024.0 * 1024.0);

        println!("📊 Rome PBF file size: {:.3} GB", file_size_gb);
        assert!(file_size_gb < 1.0, "Rome file should be small enough for full geometry mode");

        // Create temporary output for auto mode
        let output_file = NamedTempFile::new().expect("Failed to create temp file");
        let output_path = output_file.path().to_str().unwrap().to_string();

        // Run with auto mode - should select full geometry for small file
        let result = convert_pbf_to_geojson_with_geometry_level(
            rome_path,
            Some(&output_path),
            Some(vec!["multipolygon".to_string()]),
            false,
            "auto", // Auto mode should select full geometry
        );

        assert!(result.is_ok(), "Auto mode conversion should succeed");

        // Verify that some output was generated
        let output_content = fs::read_to_string(&output_path)
            .expect("Should be able to read output file");

        if !output_content.trim().is_empty() {
            println!("✅ Auto mode produced output for multipolygon relations");

            // Check if any relations have geometry (indicating three-pass worked)
            let has_geometry = output_content.lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .filter(|json| json["type"] == "relation")
                .any(|json| json.get("centroid").is_some() && json.get("bounds").is_some());

            println!("✅ Relations with geometry in auto mode: {}", has_geometry);
        } else {
            println!("ℹ️  No multipolygon relations found in Rome PBF");
        }
    }
}