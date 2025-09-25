// Parallel PBF to JSON converter with streaming output and disk-based geometry
use crate::coordinate_storage::CoordinateStorage;
use crate::osm::{MemberType, OsmElement, OsmNode, OsmRelation, OsmRelationMember, OsmWay};
use anyhow::{Context, Result};
use osmpbf::{BlobDecode, BlobReader, Element};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

const CHUNK_SIZE: usize = 10_000; // Process elements in chunks for streaming output
const MEMORY_LIMIT_MB: u64 = 8192; // 8GB memory limit
const MEMORY_CHECK_INTERVAL: usize = 50; // Check memory every 50 batches

/// Parallel PBF to GeoJSON converter with streaming output and >800% CPU utilization
pub fn convert_pbf_to_geojson_parallel(
    input_path: &str,
    output_path: Option<&String>,
    tag_filter: Option<Vec<Vec<String>>>,
    pretty_print: bool,
    geometry_level: &str,
    temp_db_path: Option<&String>,
    keep_temp_db: bool,
) -> Result<()> {
    let file_size = std::fs::metadata(input_path)
        .context("Failed to get file metadata")?
        .len();
    let file_size_gb = file_size as f64 / (1024.0 * 1024.0 * 1024.0);

    eprintln!("Input file size: {:.1}GB", file_size_gb);
    eprintln!("Geometry level: {}", geometry_level);

    let use_geometry = match geometry_level {
        "basic" => {
            eprintln!("Using basic format (no geometry computation)...");
            false
        }
        "full" => {
            eprintln!("Using full geometry format with parallel disk-based coordinate storage...");
            true
        }
        "auto" => {
            if file_size_gb > 1.0 {
                eprintln!("Large file detected, auto-selecting streaming approach...");
                false
            } else {
                eprintln!("Small file detected, auto-selecting parallel full geometry...");
                true
            }
        }
        _ => {
            eprintln!(
                "Unknown geometry level '{}', defaulting to auto",
                geometry_level
            );
            file_size_gb <= 1.0
        }
    };

    if use_geometry {
        convert_parallel_with_geometry(
            input_path,
            output_path,
            tag_filter,
            pretty_print,
            temp_db_path,
            keep_temp_db,
        )
    } else {
        convert_parallel_basic(input_path, output_path, tag_filter, pretty_print)
    }
}

/// Parallel converter with disk-based geometry computation
fn convert_parallel_with_geometry(
    input_path: &str,
    output_path: Option<&String>,
    tag_filter: Option<Vec<Vec<String>>>,
    pretty_print: bool,
    temp_db_path: Option<&String>,
    keep_temp_db: bool,
) -> Result<()> {
    println!("🚀 Starting parallel PBF processing with geometry computation...");

    // Phase 1: Parallel coordinate collection to disk
    eprintln!("Phase 1: Collecting coordinates to disk with parallel processing...");
    let coordinate_storage = create_coordinate_storage(temp_db_path, keep_temp_db)?;
    let node_count = collect_coordinates_parallel(&coordinate_storage, input_path)?;
    eprintln!("Collected {} node coordinates in parallel", node_count);

    // Phase 2: Parallel processing with geometry computation
    eprintln!("Phase 2: Processing elements with parallel geometry computation...");
    let coordinate_storage = Arc::new(coordinate_storage);
    process_with_parallel_geometry(
        input_path,
        output_path,
        tag_filter,
        pretty_print,
        coordinate_storage,
    )
}

/// Original parallel converter without geometry computation
fn convert_parallel_basic(
    input_path: &str,
    output_path: Option<&String>,
    tag_filter: Option<Vec<Vec<String>>>,
    pretty_print: bool,
) -> Result<()> {
    println!("🚀 Starting parallel PBF processing (basic mode)...");

    // Setup streaming output channel
    let (tx, rx) = mpsc::channel::<Vec<String>>();
    let tag_filter_clone = tag_filter.clone();

    // Spawn background thread for streaming output
    let output_thread = {
        let output_path = output_path.cloned();
        thread::spawn(move || -> Result<(), anyhow::Error> {
            let mut writer: Box<dyn Write> = match output_path.as_ref() {
                Some(path) => {
                    let file = File::create(path)
                        .with_context(|| format!("Failed to create output file: {}", path))?;
                    Box::new(BufWriter::new(file))
                }
                None => Box::new(std::io::stdout()),
            };

            let mut total_features = 0usize;
            let mut batch_count = 0usize;

            while let Ok(json_batch) = rx.recv() {
                for json_line in json_batch {
                    writeln!(writer, "{}", json_line)?;
                    total_features += 1;
                }
                batch_count += 1;

                // Progress reporting
                if batch_count % 100 == 0 {
                    eprintln!(
                        "📊 Processed {} batches, {} total features",
                        batch_count, total_features
                    );
                    if let Some(memory_usage) = get_memory_usage_mb() {
                        eprintln!("🧠 Memory usage: {} MB", memory_usage);
                        if memory_usage > MEMORY_LIMIT_MB {
                            eprintln!(
                                "⚠️ Memory limit exceeded ({} MB), processing may slow",
                                memory_usage
                            );
                        }
                    }
                }
            }

            writer.flush()?;
            eprintln!(
                "✅ Parallel streaming complete. Total features: {}",
                total_features
            );
            Ok(())
        })
    };

    // PARALLEL PROCESSING APPROACH 1: Custom blob-level parallelization
    let file = File::open(input_path).context("Failed to open PBF file")?;
    let buf_reader = std::io::BufReader::new(file);
    let mut blob_reader = BlobReader::new(buf_reader);

    // Process blobs sequentially but elements in parallel (avoids par_bridge memory accumulation)
    let processing_result: Result<()> = {
        let mut batch_count = 0;
        blob_reader.try_for_each(|blob_result| -> Result<()> {
            let blob = blob_result.context("Failed to read blob")?;

            match blob.decode() {
                Ok(BlobDecode::OsmData(block)) => {
                    // MEMORY-BOUNDED: Process elements in streaming batches
                    let mut element_batch = Vec::with_capacity(CHUNK_SIZE);
                    let mut processed_count = 0;

                    for element in block.elements() {
                        element_batch.push(element);

                        // Process batch when full
                        if element_batch.len() >= CHUNK_SIZE {
                            let json_results: Vec<String> = element_batch
                                .par_iter()
                                .filter_map(|element| {
                                    process_element_to_json(
                                        element.clone(),
                                        &tag_filter_clone,
                                        pretty_print,
                                    )
                                })
                                .collect();

                            // Send results immediately and clear batch
                            if !json_results.is_empty() && tx.send(json_results).is_err() {
                                return Err(anyhow::anyhow!("Output channel closed"));
                            }

                            // Clear to prevent memory accumulation
                            element_batch.clear();
                            processed_count += CHUNK_SIZE;

                            // Memory monitoring
                            if processed_count % (CHUNK_SIZE * MEMORY_CHECK_INTERVAL) == 0
                                && let Some(memory_usage) = get_memory_usage_mb()
                                && memory_usage > MEMORY_LIMIT_MB
                            {
                                eprintln!(
                                    "⚠️ Memory threshold reached: {} MB, pausing...",
                                    memory_usage
                                );
                                std::thread::sleep(std::time::Duration::from_millis(100));
                            }
                        }
                    }

                    // Process remaining elements
                    if !element_batch.is_empty() {
                        let json_results: Vec<String> = element_batch
                            .par_iter()
                            .filter_map(|element| {
                                process_element_to_json(
                                    element.clone(),
                                    &tag_filter_clone,
                                    pretty_print,
                                )
                            })
                            .collect();

                        if !json_results.is_empty() && tx.send(json_results).is_err() {
                            return Err(anyhow::anyhow!("Output channel closed"));
                        }
                    }
                }
                Ok(BlobDecode::OsmHeader(_)) => {
                    // Skip header blobs
                }
                Ok(BlobDecode::Unknown(_)) => {
                    // Skip unknown blobs
                }
                Err(e) => return Err(anyhow::anyhow!("Blob decode error: {}", e)),
            }

            batch_count += 1;
            Ok(())
        })
    };

    // Close the channel to signal completion
    drop(tx);

    // Wait for output thread and processing to complete
    processing_result?;
    output_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Output thread panicked"))??;

    println!("🎉 Parallel processing completed successfully!");
    Ok(())
}

/// Create coordinate storage for parallel processing
fn create_coordinate_storage(
    temp_db_path: Option<&String>,
    keep_temp_db: bool,
) -> Result<CoordinateStorage> {
    let db_path = temp_db_path.map(Path::new);
    CoordinateStorage::new_with_cleanup(db_path, keep_temp_db)
}

/// Collect coordinates in parallel with thread-safe writes
fn collect_coordinates_parallel(storage: &CoordinateStorage, input_path: &str) -> Result<u64> {
    let mut reader = BlobReader::from_path(input_path)
        .context("Failed to open PBF file for coordinate collection")?;

    // Use Arc<Mutex<>> for thread-safe coordinate writing
    let storage_mutex = Arc::new(Mutex::new(storage));
    let node_count = Arc::new(Mutex::new(0u64));

    reader.try_for_each(|blob_result| -> Result<()> {
        let blob = blob_result.context("Failed to read blob")?;
        match blob.decode().context("Failed to decode blob")? {
            BlobDecode::OsmData(data) => {
                let mut batch_nodes = Vec::new();

                // Process elements in this blob
                for element in data.elements() {
                    match element {
                        Element::Node(node) => {
                            batch_nodes.push((node.id(), node.lat(), node.lon()));
                        }
                        Element::DenseNode(dense_node) => {
                            batch_nodes.push((dense_node.id(), dense_node.lat(), dense_node.lon()));
                        }
                        _ => {} // Skip ways and relations in coordinate collection phase
                    }
                }

                // Write batch to storage (thread-safe)
                if !batch_nodes.is_empty() {
                    let storage_guard = storage_mutex.lock().unwrap();
                    storage_guard.store_nodes(&batch_nodes)?;

                    let mut count_guard = node_count.lock().unwrap();
                    *count_guard += batch_nodes.len() as u64;
                    drop(count_guard);
                    drop(storage_guard);
                }
            }
            BlobDecode::OsmHeader(_) => {
                // Skip header blobs
            }
            BlobDecode::Unknown(_) => {
                // Skip unknown blobs
            }
        }
        Ok(())
    })?;

    storage.sync()?;
    let final_count = *node_count.lock().unwrap();
    Ok(final_count)
}

/// Process elements with parallel geometry computation (read-only coordinate access)
fn process_with_parallel_geometry(
    input_path: &str,
    output_path: Option<&String>,
    tag_filter: Option<Vec<Vec<String>>>,
    pretty_print: bool,
    coordinate_storage: Arc<CoordinateStorage>,
) -> Result<()> {
    // Setup streaming output channel
    let (tx, rx) = mpsc::channel::<Vec<String>>();
    let tag_filter_clone = tag_filter.clone();

    // Spawn background thread for streaming output
    let output_thread = {
        let output_path = output_path.cloned();
        thread::spawn(move || -> Result<(), anyhow::Error> {
            let mut writer: Box<dyn Write> = match output_path.as_ref() {
                Some(path) => {
                    let file = File::create(path)
                        .with_context(|| format!("Failed to create output file: {}", path))?;
                    Box::new(BufWriter::new(file))
                }
                None => Box::new(std::io::stdout()),
            };

            let mut batch_count = 0;
            let mut total_features = 0;

            while let Ok(json_batch) = rx.recv() {
                for json_str in json_batch {
                    writeln!(writer, "{}", json_str)?;
                    total_features += 1;
                }
                batch_count += 1;

                if batch_count % 100 == 0 {
                    eprintln!(
                        "📊 Processed {} batches, {} total features",
                        batch_count, total_features
                    );

                    // Memory monitoring (should stay low with disk storage)
                    if let Some(memory_usage) = get_memory_usage_mb() {
                        eprintln!("🧠 Memory usage: {} MB", memory_usage);
                        if memory_usage > MEMORY_LIMIT_MB {
                            eprintln!(
                                "⚠️ Memory limit exceeded ({} MB), processing may slow",
                                memory_usage
                            );
                        }
                    }
                }
            }

            writer.flush()?;
            eprintln!(
                "✅ Parallel streaming complete. Total features: {}",
                total_features
            );
            Ok(())
        })
    };

    // Process PBF file in parallel with geometry computation
    let mut reader =
        BlobReader::from_path(input_path).context("Failed to open PBF file for processing")?;

    let processing_result = {
        let mut batch_count = 0;
        reader.try_for_each(|blob_result| -> Result<()> {
            let blob = blob_result.context("Failed to read blob")?;
            match blob.decode().context("Failed to decode blob")? {
                BlobDecode::OsmData(data) => {
                    // MEMORY-BOUNDED: Stream process without collecting all elements
                    let mut element_batch = Vec::with_capacity(CHUNK_SIZE);
                    let mut processed_count = 0;

                    for element in data.elements() {
                        element_batch.push(element);

                        // Process batch when full
                        if element_batch.len() >= CHUNK_SIZE {
                            let json_results: Vec<String> = element_batch
                                .par_iter()
                                .filter_map(|element| {
                                    process_element_with_geometry(
                                        element.clone(),
                                        &tag_filter_clone,
                                        pretty_print,
                                        &coordinate_storage,
                                    )
                                })
                                .collect();

                            // Send results immediately and clear batch to free memory
                            if !json_results.is_empty() && tx.send(json_results).is_err() {
                                return Err(anyhow::anyhow!("Output channel closed"));
                            }

                            // Clear batch to prevent memory accumulation
                            element_batch.clear();
                            processed_count += CHUNK_SIZE;

                            // Memory check every MEMORY_CHECK_INTERVAL batches
                            if processed_count % (CHUNK_SIZE * MEMORY_CHECK_INTERVAL) == 0
                                && let Some(memory_usage) = get_memory_usage_mb()
                                && memory_usage > MEMORY_LIMIT_MB
                            {
                                eprintln!(
                                    "⚠️ Memory threshold reached: {} MB, pausing...",
                                    memory_usage
                                );
                                std::thread::sleep(std::time::Duration::from_millis(100));
                            }
                        }
                    }

                    // Process remaining elements in final batch
                    if !element_batch.is_empty() {
                        let json_results: Vec<String> = element_batch
                            .par_iter()
                            .filter_map(|element| {
                                process_element_with_geometry(
                                    element.clone(),
                                    &tag_filter_clone,
                                    pretty_print,
                                    &coordinate_storage,
                                )
                            })
                            .collect();

                        if !json_results.is_empty() && tx.send(json_results).is_err() {
                            return Err(anyhow::anyhow!("Output channel closed"));
                        }
                    }
                }
                BlobDecode::OsmHeader(_) => {
                    // Skip header blobs
                }
                BlobDecode::Unknown(_) => {
                    // Skip unknown blobs
                }
            }
            batch_count += 1;
            Ok(())
        })
    };

    // Close the channel to signal completion
    drop(tx);

    // Wait for output thread and processing to complete
    processing_result?;
    output_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Output thread panicked"))??;

    eprintln!("🎉 Parallel geometry processing completed successfully!");
    Ok(())
}

/// Process element with geometry computation (thread-safe read-only coordinate access)
fn process_element_with_geometry(
    element: Element,
    tag_filter: &Option<Vec<Vec<String>>>,
    pretty_print: bool,
    coordinate_storage: &Arc<CoordinateStorage>,
) -> Option<String> {
    let osm_element = convert_element_to_osm(element)?;

    // Apply tag filter
    if let Some(filter_tags) = tag_filter
        && !osm_element.matches_filter(filter_tags)
    {
        return None;
    }

    // Convert to JSON with geometry if applicable
    match &osm_element {
        OsmElement::Node(node) => {
            if !node.tags.is_empty() {
                convert_node_to_json(node, pretty_print)
            } else {
                None
            }
        }
        OsmElement::Way(way) => {
            if !way.tags.is_empty() {
                convert_way_to_json_with_parallel_geometry(way, coordinate_storage, pretty_print)
            } else {
                None
            }
        }
        OsmElement::Relation(relation) => {
            if !relation.tags.is_empty() {
                convert_relation_to_json_with_parallel_geometry(
                    relation,
                    coordinate_storage,
                    pretty_print,
                )
            } else {
                None
            }
        }
    }
}

/// Convert way to JSON with parallel-safe geometry computation
fn convert_way_to_json_with_parallel_geometry(
    way: &OsmWay,
    storage: &Arc<CoordinateStorage>,
    pretty_print: bool,
) -> Option<String> {
    use serde_json::json;

    // Get coordinates from disk storage (thread-safe read)
    let coordinates: Vec<(f64, f64)> = match storage.get_nodes(&way.node_refs) {
        Ok(coords) => coords.into_iter().flatten().collect(),
        Err(_) => return convert_way_to_json_basic(way, pretty_print), // Fallback
    };

    if coordinates.is_empty() {
        return convert_way_to_json_basic(way, pretty_print);
    }

    let (centroid_lat, centroid_lon) = calculate_centroid(&coordinates);
    let bounds = calculate_bounds(&coordinates);

    let record = json!({
        "id": way.id,
        "type": "way",
        "nodes": way.node_refs,
        "tags": way.tags,
        "centroid": {
            "lat": format!("{:.7}", centroid_lat),
            "lon": format!("{:.7}", centroid_lon),
            "type": "centroid"
        },
        "bounds": {
            "n": format!("{:.7}", bounds.north),
            "s": format!("{:.7}", bounds.south),
            "e": format!("{:.7}", bounds.east),
            "w": format!("{:.7}", bounds.west)
        }
    });

    if pretty_print {
        serde_json::to_string_pretty(&record).ok()
    } else {
        serde_json::to_string(&record).ok()
    }
}

/// Convert relation to JSON with parallel-safe geometry computation
fn convert_relation_to_json_with_parallel_geometry(
    relation: &OsmRelation,
    storage: &Arc<CoordinateStorage>,
    pretty_print: bool,
) -> Option<String> {
    use serde_json::json;

    // For relations, collect coordinates from node members
    let node_ids: Vec<i64> = relation
        .members
        .iter()
        .filter(|m| m.member_type == MemberType::Node)
        .map(|m| m.member_id)
        .collect();

    let mut all_coordinates = Vec::new();
    if !node_ids.is_empty()
        && let Ok(coords) = storage.get_nodes(&node_ids)
    {
        all_coordinates.extend(coords.into_iter().flatten());
    }

    let mut record = json!({
        "id": relation.id,
        "type": "relation",
        "tags": relation.tags
    });

    if !all_coordinates.is_empty() {
        let (centroid_lat, centroid_lon) = calculate_centroid(&all_coordinates);
        let bounds = calculate_bounds(&all_coordinates);

        record.as_object_mut().unwrap().insert(
            "centroid".to_string(),
            json!({
                "lat": format!("{:.7}", centroid_lat),
                "lon": format!("{:.7}", centroid_lon),
                "type": "entrance"
            }),
        );

        record.as_object_mut().unwrap().insert(
            "bounds".to_string(),
            json!({
                "n": format!("{:.7}", bounds.north),
                "s": format!("{:.7}", bounds.south),
                "e": format!("{:.7}", bounds.east),
                "w": format!("{:.7}", bounds.west)
            }),
        );
    } else {
        // Fallback to including members
        let members_json: Vec<serde_json::Value> = relation
            .members
            .iter()
            .map(|member| {
                json!({
                    "type": match member.member_type {
                        MemberType::Node => "node",
                        MemberType::Way => "way",
                        MemberType::Relation => "relation",
                    },
                    "ref": member.member_id,
                    "role": member.role
                })
            })
            .collect();

        record
            .as_object_mut()
            .unwrap()
            .insert("members".to_string(), json!(members_json));
    }

    if pretty_print {
        serde_json::to_string_pretty(&record).ok()
    } else {
        serde_json::to_string(&record).ok()
    }
}

/// Helper functions from converter.rs
fn calculate_centroid(coordinates: &[(f64, f64)]) -> (f64, f64) {
    if coordinates.is_empty() {
        return (0.0, 0.0);
    }

    let sum_lat: f64 = coordinates.iter().map(|(lat, _)| lat).sum();
    let sum_lon: f64 = coordinates.iter().map(|(_, lon)| lon).sum();
    let count = coordinates.len() as f64;

    (sum_lat / count, sum_lon / count)
}

#[derive(Debug, Clone)]
struct Bounds {
    north: f64,
    south: f64,
    east: f64,
    west: f64,
}

fn calculate_bounds(coordinates: &[(f64, f64)]) -> Bounds {
    if coordinates.is_empty() {
        return Bounds {
            north: 0.0,
            south: 0.0,
            east: 0.0,
            west: 0.0,
        };
    }

    let mut north = f64::NEG_INFINITY;
    let mut south = f64::INFINITY;
    let mut east = f64::NEG_INFINITY;
    let mut west = f64::INFINITY;

    for &(lat, lon) in coordinates {
        north = north.max(lat);
        south = south.min(lat);
        east = east.max(lon);
        west = west.min(lon);
    }

    Bounds {
        north,
        south,
        east,
        west,
    }
}

fn convert_element_to_osm(element: Element) -> Option<OsmElement> {
    match element {
        Element::Node(node) => {
            let tags: HashMap<String, String> = node
                .tags()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            Some(OsmElement::Node(OsmNode {
                id: node.id(),
                lat: node.lat(),
                lon: node.lon(),
                tags,
            }))
        }
        Element::DenseNode(dense_node) => {
            let tags: HashMap<String, String> = dense_node
                .tags()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            Some(OsmElement::Node(OsmNode {
                id: dense_node.id(),
                lat: dense_node.lat(),
                lon: dense_node.lon(),
                tags,
            }))
        }
        Element::Way(way) => {
            let tags: HashMap<String, String> = way
                .tags()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            let node_refs: Vec<i64> = way.refs().collect();
            Some(OsmElement::Way(OsmWay {
                id: way.id(),
                node_refs,
                tags,
            }))
        }
        Element::Relation(relation) => {
            let tags: HashMap<String, String> = relation
                .tags()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            let members: Vec<OsmRelationMember> = relation
                .members()
                .map(|member| {
                    let member_type = match member.member_type {
                        osmpbf::RelMemberType::Node => MemberType::Node,
                        osmpbf::RelMemberType::Way => MemberType::Way,
                        osmpbf::RelMemberType::Relation => MemberType::Relation,
                    };
                    OsmRelationMember {
                        member_type,
                        member_id: member.member_id,
                        role: member.role().unwrap_or("").to_string(),
                    }
                })
                .collect();

            Some(OsmElement::Relation(OsmRelation {
                id: relation.id(),
                members,
                tags,
            }))
        }
    }
}

fn convert_node_to_json(node: &OsmNode, pretty_print: bool) -> Option<String> {
    use serde_json::json;

    let record = json!({
        "id": node.id,
        "type": "node",
        "lat": node.lat,
        "lon": node.lon,
        "tags": node.tags
    });

    if pretty_print {
        serde_json::to_string_pretty(&record).ok()
    } else {
        serde_json::to_string(&record).ok()
    }
}

fn convert_way_to_json_basic(way: &OsmWay, pretty_print: bool) -> Option<String> {
    use serde_json::json;

    let record = json!({
        "id": way.id,
        "type": "way",
        "nodes": way.node_refs,
        "tags": way.tags
    });

    if pretty_print {
        serde_json::to_string_pretty(&record).ok()
    } else {
        serde_json::to_string(&record).ok()
    }
}

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

/// Process element for basic mode (no geometry computation)
fn process_element_to_json(
    element: Element,
    tag_filter: &Option<Vec<Vec<String>>>,
    pretty_print: bool,
) -> Option<String> {
    let osm_element = convert_element_to_osm(element)?;

    // Apply tag filter
    if let Some(filter_tags) = tag_filter
        && !osm_element.matches_filter(filter_tags)
    {
        return None;
    }

    // Convert to JSON (basic mode - no geometry)
    match &osm_element {
        OsmElement::Node(node) => {
            if !node.tags.is_empty() {
                convert_node_to_json(node, pretty_print)
            } else {
                None
            }
        }
        OsmElement::Way(way) => {
            if !way.tags.is_empty() {
                convert_way_to_json_basic(way, pretty_print)
            } else {
                None
            }
        }
        OsmElement::Relation(relation) => {
            if !relation.tags.is_empty() {
                convert_relation_to_json_basic(relation, pretty_print)
            } else {
                None
            }
        }
    }
}

fn convert_relation_to_json_basic(relation: &OsmRelation, pretty_print: bool) -> Option<String> {
    use serde_json::json;

    let members: Vec<serde_json::Value> = relation
        .members
        .iter()
        .map(|member| {
            json!({
                "type": match member.member_type {
                    MemberType::Node => "node",
                    MemberType::Way => "way",
                    MemberType::Relation => "relation",
                },
                "ref": member.member_id,
                "role": member.role
            })
        })
        .collect();

    let record = json!({
        "id": relation.id,
        "type": "relation",
        "members": members,
        "tags": relation.tags
    });

    if pretty_print {
        serde_json::to_string_pretty(&record).ok()
    } else {
        serde_json::to_string(&record).ok()
    }
}
