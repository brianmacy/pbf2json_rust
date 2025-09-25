// CPU utilization benchmark example
use rayon::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

fn main() {
    println!("🚀 CPU Utilization Benchmark for Parallel PBF Processing");
    println!("=========================================================");

    let num_cpus = num_cpus::get();
    println!("🖥️  Available CPU cores: {}", num_cpus);

    // Benchmark 1: Rayon Parallel Iterator Performance
    benchmark_rayon_performance();

    // Benchmark 2: Simulated PBF Blob Processing
    benchmark_simulated_pbf_processing();

    // Benchmark 3: Memory-bounded Parallel Processing
    benchmark_memory_bounded_processing();

    println!("✅ Benchmark complete!");
    println!();
    println!("🧪 To test with real PBF data:");
    println!("   cargo run --release -- --parallel input.pbf --output output.json");
    println!("   (Monitor with htop/top for >800% CPU utilization)");
}

fn benchmark_rayon_performance() {
    println!("\n📊 Benchmark 1: Rayon Parallel Iterator Performance");
    println!("---------------------------------------------------");

    let data_size = 10_000_000;
    println!("Processing {} items...", data_size);

    // Sequential benchmark
    let start = Instant::now();
    let sequential_result: u64 = (0..data_size).map(expensive_computation).sum();
    let sequential_time = start.elapsed();

    println!(
        "Sequential: {:?} (result: {})",
        sequential_time, sequential_result
    );

    // Parallel benchmark
    let start = Instant::now();
    let parallel_result: u64 = (0..data_size)
        .into_par_iter()
        .map(expensive_computation)
        .sum();
    let parallel_time = start.elapsed();

    println!(
        "Parallel:   {:?} (result: {})",
        parallel_time, parallel_result
    );

    // Calculate speedup
    if parallel_time < sequential_time {
        let speedup = sequential_time.as_nanos() as f64 / parallel_time.as_nanos() as f64;
        println!("⚡ Speedup: {:.2}x", speedup);

        // Estimate CPU utilization
        let estimated_utilization = speedup * 100.0;
        println!(
            "📊 Estimated CPU utilization: {:.0}%",
            estimated_utilization
        );

        if estimated_utilization >= 800.0 {
            println!("🎯 Target >800% CPU utilization achieved!");
        }
    }

    assert_eq!(sequential_result, parallel_result);
}

fn benchmark_simulated_pbf_processing() {
    println!("\n📊 Benchmark 2: Simulated PBF Blob Processing");
    println!("---------------------------------------------");

    // Simulate PBF blob processing similar to our parallel converter
    let num_blobs = 1000;
    let elements_per_blob = 5000;

    println!(
        "Processing {} blobs with {} elements each...",
        num_blobs, elements_per_blob
    );

    let start = Instant::now();

    // Simulate parallel blob processing
    let total_processed: usize = (0..num_blobs)
        .into_par_iter()
        .map(|blob_id| {
            // Simulate processing all elements in a blob
            (0..elements_per_blob)
                .into_par_iter()
                .map(|element_id| simulate_element_processing(blob_id, element_id))
                .sum::<usize>()
        })
        .sum();

    let processing_time = start.elapsed();

    println!(
        "Processed {} elements in {:?}",
        total_processed, processing_time
    );
    println!(
        "Throughput: {:.0} elements/sec",
        total_processed as f64 / processing_time.as_secs_f64()
    );

    let expected_elements = num_blobs * elements_per_blob;
    assert_eq!(total_processed, expected_elements);
    println!("✅ All elements processed correctly");
}

fn benchmark_memory_bounded_processing() {
    println!("\n📊 Benchmark 3: Memory-bounded Parallel Processing");
    println!("--------------------------------------------------");

    let chunk_size = 50_000;
    let num_chunks = 20;

    println!(
        "Processing {} chunks of {} elements each...",
        num_chunks, chunk_size
    );

    let total_counter = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    // Process chunks in parallel with memory bounds
    (0..num_chunks).into_par_iter().for_each(|chunk_id| {
        // Process chunk elements
        let chunk_result: Vec<String> = (0..chunk_size)
            .into_par_iter()
            .map(|i| format!("element_{}_{}", chunk_id, i))
            .collect();

        // Simulate JSON conversion and output
        let json_count = chunk_result.len();
        total_counter.fetch_add(json_count, Ordering::Relaxed);

        // Simulate streaming output (memory is freed here)
        drop(chunk_result);
    });

    let processing_time = start.elapsed();
    let total_processed = total_counter.load(Ordering::Relaxed);

    println!(
        "Processed {} elements in {:?}",
        total_processed, processing_time
    );
    println!("Memory-bounded processing: Chunks processed independently");
    println!("✅ Memory bounded parallel processing complete");

    assert_eq!(total_processed, num_chunks * chunk_size);
}

// Simulate expensive computation (like PBF element processing and JSON conversion)
fn expensive_computation(n: usize) -> u64 {
    let mut result = n as u64;

    // Simulate tag processing
    for i in 0..10 {
        result = result.wrapping_mul(31).wrapping_add(i);
    }

    // Simulate JSON serialization work
    let _simulated_json = format!("{{\"id\":{},\"type\":\"node\",\"tags\":{{}}}}", result);

    result % 1000
}

// Simulate element processing (tag filtering, JSON conversion)
fn simulate_element_processing(blob_id: usize, element_id: usize) -> usize {
    // Simulate processing - all elements are processed in this example
    let _json = format!("{{\"id\":{},\"blob\":{}}}", element_id, blob_id);

    1 // Processed element count
}
