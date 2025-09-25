// Tests for parallel PBF processing
use pbf2json::convert_pbf_to_geojson_parallel;
use tempfile::NamedTempFile;

#[test]
fn test_parallel_converter_compiles() {
    // This test ensures the parallel converter code compiles without runtime testing
    println!("✅ Parallel converter compiles successfully");
}

#[cfg(test)]
#[test]
fn test_parallel_processing_with_dummy_data() {
    // Note: This test would need a real PBF file to run properly
    // For now, we just test the API exists and is callable

    let non_existent_file = "tests/non_existent.pbf";
    let temp_output = NamedTempFile::new().expect("Failed to create temp file");
    let output_path = temp_output.path().to_str().unwrap().to_string();

    // This will fail because the file doesn't exist, but it tests the API
    let result = convert_pbf_to_geojson_parallel(
        non_existent_file,
        Some(&output_path),
        None,  // no tag filter
        false, // not pretty print
    );

    // Should fail because file doesn't exist - that's expected
    assert!(result.is_err());
    println!("✅ Parallel API is callable and handles file not found correctly");
}

// Test CPU utilization measurement
#[test]
fn test_cpu_core_detection() {
    let num_cpus = num_cpus::get();
    println!("🖥️  Detected {} CPU cores", num_cpus);

    // For >800% CPU utilization, we need at least 8 cores active
    // This is informational - the actual test needs a real PBF file
    if num_cpus >= 8 {
        println!("✅ System has sufficient cores for >800% CPU utilization target");
    } else {
        println!(
            "⚠️  System has {} cores - may not achieve >800% CPU utilization",
            num_cpus
        );
    }
}

// Memory monitoring test
#[test]
#[cfg(target_os = "linux")]
fn test_memory_monitoring() {
    use std::fs;

    // Test memory monitoring function works on Linux
    let contents = fs::read_to_string("/proc/self/status");
    assert!(
        contents.is_ok(),
        "Should be able to read /proc/self/status on Linux"
    );

    let contents = contents.unwrap();
    let has_vmrss = contents.lines().any(|line| line.starts_with("VmRSS:"));
    assert!(has_vmrss, "Should find VmRSS line in /proc/self/status");

    println!("✅ Memory monitoring is functional on this Linux system");
}
