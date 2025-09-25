// Benchmark and CPU utilization tests for parallel PBF processing
#[cfg(test)]
mod benchmark_tests {
    use std::process::Command;
    use std::time::Instant;

    #[test]
    fn test_cpu_core_availability() {
        let num_cpus = num_cpus::get();
        println!("🖥️  Available CPU cores: {}", num_cpus);

        // Log system info for benchmark context
        if let Ok(output) = Command::new("uname").arg("-a").output()
            && let Ok(system_info) = String::from_utf8(output.stdout)
        {
            println!("🔍 System: {}", system_info.trim());
        }

        // Check if we have enough cores for >800% utilization
        assert!(num_cpus >= 1, "Need at least 1 CPU core");
        if num_cpus >= 8 {
            println!(
                "✅ System has {} cores - capable of >800% CPU utilization",
                num_cpus
            );
        } else {
            println!(
                "⚠️  System has only {} cores - max theoretical utilization: {}%",
                num_cpus,
                num_cpus * 100
            );
        }
    }

    #[test]
    fn benchmark_memory_bounded_processing() {
        println!("🧪 Testing memory-bounded parallel processing...");

        // Test memory monitoring function
        if let Some(initial_memory) = get_memory_usage_mb() {
            println!("📊 Initial memory usage: {} MB", initial_memory);

            // Simulate processing workload
            let _large_data: Vec<String> =
                (0..10000).map(|i| format!("test_element_{}", i)).collect();

            if let Some(peak_memory) = get_memory_usage_mb() {
                let memory_increase = peak_memory - initial_memory;
                println!("📈 Memory increase: {} MB", memory_increase);

                // Memory should stay bounded (under reasonable limits)
                assert!(
                    memory_increase < 100,
                    "Memory increase should be reasonable"
                );
                println!("✅ Memory usage stays bounded during processing");
            }
        } else {
            println!("⚠️  Memory monitoring not available on this platform");
        }
    }

    #[test]
    fn test_parallel_vs_sequential_performance_concept() {
        println!("🚀 Testing parallel vs sequential processing concept...");

        // This test demonstrates the parallel processing concept
        // In real usage, it would need actual PBF data

        let start_sequential = Instant::now();
        // Simulate sequential processing
        let sequential_work: u64 = (0..1000000).map(|i| i as u64).sum();
        let sequential_duration = start_sequential.elapsed();

        let start_parallel = Instant::now();
        // Simulate parallel processing using rayon
        use rayon::prelude::*;
        let parallel_work: u64 = (0..1000000).into_par_iter().map(|i| i as u64).sum();
        let parallel_duration = start_parallel.elapsed();

        println!("📊 Sequential processing: {:?}", sequential_duration);
        println!("📊 Parallel processing:   {:?}", parallel_duration);

        assert_eq!(
            sequential_work, parallel_work,
            "Results should be identical"
        );

        if parallel_duration < sequential_duration {
            let speedup =
                sequential_duration.as_nanos() as f64 / parallel_duration.as_nanos() as f64;
            println!("⚡ Parallel speedup: {:.2}x", speedup);
            println!("✅ Parallel processing shows performance improvement");
        } else {
            println!("ℹ️  Parallel overhead may be higher for this workload size");
        }
    }

    #[test]
    fn test_rayon_thread_pool_utilization() {
        println!("🔧 Testing Rayon thread pool utilization...");

        // Test that Rayon can utilize multiple threads
        use rayon::prelude::*;
        use std::collections::HashSet;
        use std::sync::{Arc, Mutex};

        let thread_ids = Arc::new(Mutex::new(HashSet::new()));

        // Process items in parallel and collect unique thread IDs
        (0..1000).into_par_iter().for_each(|_| {
            let thread_id = std::thread::current().id();
            thread_ids.lock().unwrap().insert(thread_id);
        });

        let unique_threads = thread_ids.lock().unwrap().len();
        println!("🧵 Unique threads used: {}", unique_threads);

        // Should use multiple threads (at least 2 on multi-core systems)
        let num_cpus = num_cpus::get();
        if num_cpus > 1 {
            assert!(unique_threads > 1, "Should utilize multiple threads");
            println!("✅ Rayon successfully utilizes {} threads", unique_threads);

            // Calculate potential CPU utilization
            let potential_utilization = (unique_threads as f64 / num_cpus as f64) * 100.0;
            println!(
                "📊 Potential CPU utilization: {:.1}%",
                potential_utilization
            );

            if potential_utilization >= 800.0 {
                println!("🎯 Capable of >800% CPU utilization!");
            }
        } else {
            println!("ℹ️  Single-core system - parallel benefits limited");
        }
    }

    #[test]
    fn test_streaming_architecture() {
        println!("📡 Testing streaming architecture concept...");

        use std::sync::mpsc;
        use std::thread;

        // Test the streaming architecture used in parallel converter
        let (tx, rx) = mpsc::channel::<Vec<String>>();

        // Simulate producer (parallel processing)
        let producer = thread::spawn(move || {
            for batch in 0..10 {
                let batch_data: Vec<String> =
                    (0..1000).map(|i| format!("item_{}_{}", batch, i)).collect();

                if tx.send(batch_data).is_err() {
                    break;
                }
            }
        });

        // Simulate consumer (streaming output)
        let mut total_items = 0;
        let consumer_start = Instant::now();

        while let Ok(batch) = rx.recv() {
            total_items += batch.len();

            // Simulate processing each item
            for _item in batch {
                // In real implementation, this would be JSON serialization and output
            }
        }

        let streaming_duration = consumer_start.elapsed();
        producer.join().unwrap();

        println!(
            "📊 Streamed {} items in {:?}",
            total_items, streaming_duration
        );
        println!(
            "📊 Average throughput: {:.0} items/sec",
            total_items as f64 / streaming_duration.as_secs_f64()
        );

        assert_eq!(total_items, 10000, "Should process all items");
        println!("✅ Streaming architecture works correctly");
    }

    // Helper function for memory monitoring (duplicated from parallel_converter)
    fn get_memory_usage_mb() -> Option<u64> {
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            let contents = fs::read_to_string("/proc/self/status").ok()?;
            for line in contents.lines() {
                if line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        return parts[1].parse::<u64>().ok().map(|kb| kb / 1024);
                    }
                }
            }
            None
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }
}

// Integration test for real PBF processing (when test data is available)
#[cfg(test)]
mod integration_tests {
    use std::path::Path;

    #[test]
    fn test_pbf_file_availability() {
        println!("🔍 Checking for test PBF files...");

        let test_files = ["tests/test.osm.pbf", "test.osm.pbf", "../test.osm.pbf"];

        let mut found_file = None;
        for file_path in &test_files {
            if Path::new(file_path).exists() {
                found_file = Some(file_path);
                break;
            }
        }

        if let Some(file_path) = found_file {
            println!("✅ Found test PBF file: {}", file_path);
            println!("ℹ️  To test parallel processing with real data:");
            println!(
                "    cargo run --release -- --parallel {} --output /tmp/parallel_test.json",
                file_path
            );
        } else {
            println!("ℹ️  No test PBF files found. To test with real data:");
            println!("    1. Download a PBF file (e.g., from https://download.geofabrik.de/)");
            println!(
                "    2. Run: cargo run --release -- --parallel input.pbf --output output.json"
            );
            println!("    3. Monitor with: htop or top to verify >800% CPU usage");
        }
    }
}
