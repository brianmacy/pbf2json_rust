use crate::osm::{MemberType, OsmElement, OsmNode, OsmRelation, OsmRelationMember, OsmWay};
use anyhow::{Context, Result};
use osmpbf::{Element, ElementReader};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::mpsc;
use std::thread;

const MEMORY_LIMIT_GB: u64 = 8;

pub fn convert_pbf_to_geojson_with_geometry_level(
    input_path: &str,
    output_path: Option<&String>,
    tag_filter: Option<Vec<Vec<String>>>,
    pretty_print: bool,
    geometry_level: &str,
) -> Result<()> {
    let file_size = std::fs::metadata(input_path)
        .context("Failed to get file metadata")?
        .len();
    let file_size_gb = file_size as f64 / (1024.0 * 1024.0 * 1024.0);

    eprintln!("Input file size: {:.1}GB", file_size_gb);
    eprintln!("Geometry level: {}", geometry_level);

    let use_full_geometry = match geometry_level {
        "basic" => {
            eprintln!("Using basic format (no geometry computation)...");
            false
        }
        "full" => {
            eprintln!("Using full geometry format (may require significant memory)...");
            true
        }
        "auto" => {
            if file_size_gb > 1.0 {
                eprintln!("Large file detected, auto-selecting streaming approach...");
                false
            } else {
                eprintln!("Small file detected, auto-selecting full geometry...");
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

    if use_full_geometry {
        // For very small files, attempt three-pass processing for complete relation geometry
        if file_size_gb < 0.1 {
            eprintln!(
                "Very small file, attempting three-pass processing with relation geometry..."
            );
            convert_pbf_with_complete_geometry(input_path, output_path, tag_filter, pretty_print)
        } else {
            convert_pbf_with_full_geometry(input_path, output_path, tag_filter, pretty_print)
        }
    } else {
        convert_pbf_streaming_only(input_path, output_path, tag_filter, pretty_print)
    }
}

fn convert_pbf_with_full_geometry(
    input_path: &str,
    output_path: Option<&String>,
    tag_filter: Option<Vec<Vec<String>>>,
    pretty_print: bool,
) -> Result<()> {
    // TWO-PASS APPROACH for complete pbf2json compatibility
    eprintln!("Pass 1: Collecting all node coordinates...");
    let all_nodes = collect_all_nodes(input_path)?;
    eprintln!(
        "Collected {} node coordinates ({:.1}MB memory)",
        all_nodes.len(),
        all_nodes.len() as f64 * 16.0 / 1_048_576.0
    );

    eprintln!("Pass 2: Processing elements with full geometry...");
    let reader = ElementReader::from_path(input_path).context("Failed to open PBF file")?;

    // Streaming architecture with complete geometry computation
    let (tx, rx) = mpsc::sync_channel::<String>(1000);
    let tag_filter_clone = tag_filter.clone();
    let all_nodes_clone = all_nodes;

    // Spawn background thread for immediate output streaming
    let output_thread = {
        let output_path = output_path.cloned();
        thread::spawn(move || -> Result<(), anyhow::Error> {
            // Setup output writer in the output thread
            let mut writer: Box<dyn Write> = match output_path.as_ref() {
                Some(path) => {
                    let file = File::create(path)
                        .with_context(|| format!("Failed to create output file: {}", path))?;
                    Box::new(BufWriter::new(file))
                }
                None => Box::new(std::io::stdout()),
            };

            let mut feature_count = 0usize;

            while let Ok(json_line) = rx.recv() {
                writeln!(writer, "{}", json_line)?; // Stream immediately to output
                feature_count += 1;

                // Memory monitoring every 10k features
                if feature_count % 10000 == 0 {
                    eprintln!("Streamed {} features", feature_count);
                    if let Some(memory_usage) = get_memory_usage_mb() {
                        eprintln!("Current memory usage: {} MB", memory_usage);
                    }
                }

                // Memory warning
                if feature_count % 50000 == 0
                    && let Some(memory_usage) = get_memory_usage_mb()
                    && memory_usage > MEMORY_LIMIT_GB * 1024
                {
                    eprintln!(
                        "⚠️  Memory usage ({} MB) exceeds limit ({} GB)",
                        memory_usage, MEMORY_LIMIT_GB
                    );
                }
            }

            writer.flush()?;
            eprintln!(
                "Streaming output complete. Total features: {}",
                feature_count
            );
            Ok(())
        })
    };

    // PARALLEL PROCESSING: Use par_map_reduce for multi-core processing
    reader.par_map_reduce(
        |element| {
            // Parallel map: Process each element on available CPU cores

            let mut results = Vec::new();
            if let Some(osm_element) = process_element(element, &tag_filter_clone) {
                let json_opt = match &osm_element {
                    OsmElement::Node(node) => {
                        if !node.tags.is_empty() {
                            convert_node_to_json(node, pretty_print)
                        } else {
                            None
                        }
                    }
                    OsmElement::Way(way) => {
                        if !way.tags.is_empty() {
                            convert_way_to_json_with_full_geometry(
                                way,
                                &all_nodes_clone,
                                pretty_print,
                            )
                        } else {
                            None
                        }
                    }
                    OsmElement::Relation(relation) => {
                        if !relation.tags.is_empty() {
                            convert_relation_to_json_with_full_geometry(
                                relation,
                                &all_nodes_clone,
                                pretty_print,
                            )
                        } else {
                            None
                        }
                    }
                };

                if let Some(json_str) = json_opt
                    && !json_str.is_empty()
                {
                    results.push(json_str);
                }
            }
            results
        },
        Vec::new,
        |mut acc, mut batch| {
            // Reduce: Stream results immediately to output thread
            for json_str in batch.drain(..) {
                if tx.send(json_str).is_err() {
                    return acc; // Output thread disconnected
                }
            }

            // Keep memory bounded - don't accumulate
            acc.clear();
            acc
        },
    )?;

    // Close channel to signal completion
    drop(tx);

    // Wait for output thread to finish
    output_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Output thread panicked"))??;

    Ok(())
}

fn process_element(element: Element, tag_filter: &Option<Vec<Vec<String>>>) -> Option<OsmElement> {
    let osm_element = match element {
        Element::Node(node) => {
            let tags: HashMap<String, String> = node
                .tags()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            OsmElement::Node(OsmNode {
                id: node.id(),
                lat: node.lat(),
                lon: node.lon(),
                tags,
            })
        }
        Element::DenseNode(dense_node) => {
            let tags: HashMap<String, String> = dense_node
                .tags()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            OsmElement::Node(OsmNode {
                id: dense_node.id(),
                lat: dense_node.lat(),
                lon: dense_node.lon(),
                tags,
            })
        }
        Element::Way(way) => {
            let tags: HashMap<String, String> = way
                .tags()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            let node_refs: Vec<i64> = way.refs().collect();
            OsmElement::Way(OsmWay {
                id: way.id(),
                node_refs,
                tags,
            })
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

            OsmElement::Relation(OsmRelation {
                id: relation.id(),
                members,
                tags,
            })
        }
    };

    if let Some(filter_tags) = tag_filter {
        if osm_element.matches_filter(filter_tags) {
            Some(osm_element)
        } else {
            None
        }
    } else {
        Some(osm_element)
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

fn convert_way_to_json(way: &OsmWay, pretty_print: bool) -> Option<String> {
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

fn convert_relation_to_json(relation: &OsmRelation, pretty_print: bool) -> Option<String> {
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

fn collect_all_nodes(input_path: &str) -> Result<HashMap<i64, (f64, f64)>> {
    let reader = ElementReader::from_path(input_path)
        .context("Failed to open PBF file for node collection")?;

    // PARALLEL NODE COLLECTION: Use par_map_reduce for multi-core node processing
    let nodes = reader.par_map_reduce(
        |element| {
            // Parallel map: Process elements on available CPU cores

            let mut local_nodes = HashMap::new();
            match element {
                Element::Node(node) => {
                    local_nodes.insert(node.id(), (node.lat(), node.lon()));
                }
                Element::DenseNode(dense_node) => {
                    local_nodes.insert(dense_node.id(), (dense_node.lat(), dense_node.lon()));
                }
                _ => {} // Skip ways and relations in pass 1
            }
            local_nodes
        },
        HashMap::new,
        |mut acc, batch| {
            // Reduce: Merge node collections
            acc.extend(batch);
            acc
        },
    )?;

    Ok(nodes)
}

fn convert_pbf_streaming_only(
    input_path: &str,
    output_path: Option<&String>,
    tag_filter: Option<Vec<Vec<String>>>,
    pretty_print: bool,
) -> Result<()> {
    // SINGLE-PASS STREAMING for large files (no geometry computation)
    eprintln!("Single-pass streaming processing (basic format without full geometry)...");
    let reader = ElementReader::from_path(input_path).context("Failed to open PBF file")?;

    // Streaming architecture without geometry computation
    let (tx, rx) = mpsc::sync_channel::<String>(1000);
    let tag_filter_clone = tag_filter.clone();

    // Spawn background thread for immediate output streaming
    let output_thread = {
        let output_path = output_path.cloned();
        thread::spawn(move || -> Result<(), anyhow::Error> {
            // Setup output writer in the output thread
            let mut writer: Box<dyn Write> = match output_path.as_ref() {
                Some(path) => {
                    let file = File::create(path)
                        .with_context(|| format!("Failed to create output file: {}", path))?;
                    Box::new(BufWriter::new(file))
                }
                None => Box::new(std::io::stdout()),
            };

            let mut feature_count = 0usize;

            while let Ok(json_line) = rx.recv() {
                writeln!(writer, "{}", json_line)?; // Stream immediately to output
                feature_count += 1;

                // Memory monitoring every 100k features for large files
                if feature_count % 100000 == 0 {
                    eprintln!("Streamed {} features", feature_count);
                    if let Some(memory_usage) = get_memory_usage_mb() {
                        eprintln!("Current memory usage: {} MB", memory_usage);
                    }
                }
            }

            writer.flush()?;
            eprintln!(
                "Streaming output complete. Total features: {}",
                feature_count
            );
            Ok(())
        })
    };

    // PARALLEL PROCESSING: Basic format without geometry computation
    reader.par_map_reduce(
        |element| {
            // Parallel map: Process each element on available CPU cores
            let mut results = Vec::new();
            if let Some(osm_element) = process_element(element, &tag_filter_clone) {
                let json_opt = match &osm_element {
                    OsmElement::Node(node) => {
                        if !node.tags.is_empty() {
                            convert_node_to_json(node, pretty_print)
                        } else {
                            None
                        }
                    }
                    OsmElement::Way(way) => {
                        if !way.tags.is_empty() {
                            // Basic format without geometry for large files
                            convert_way_to_json(way, pretty_print)
                        } else {
                            None
                        }
                    }
                    OsmElement::Relation(relation) => {
                        if !relation.tags.is_empty() {
                            // Basic format without geometry for large files
                            convert_relation_to_json(relation, pretty_print)
                        } else {
                            None
                        }
                    }
                };

                if let Some(json_str) = json_opt
                    && !json_str.is_empty()
                {
                    results.push(json_str);
                }
            }
            results
        },
        Vec::new,
        |mut acc, mut batch| {
            // Reduce: Stream results immediately to output thread
            for json_str in batch.drain(..) {
                if tx.send(json_str).is_err() {
                    return acc; // Output thread disconnected
                }
            }

            // Keep memory bounded - don't accumulate
            acc.clear();
            acc
        },
    )?;

    // Close channel to signal completion
    drop(tx);

    // Wait for output thread to finish
    output_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Output thread panicked"))??;

    Ok(())
}

fn convert_pbf_with_complete_geometry(
    input_path: &str,
    output_path: Option<&String>,
    tag_filter: Option<Vec<Vec<String>>>,
    pretty_print: bool,
) -> Result<()> {
    // THREE-PASS APPROACH for complete relation geometry (small files only)
    eprintln!("Pass 1: Collecting all node coordinates...");
    let all_nodes = collect_all_nodes(input_path)?;
    eprintln!(
        "Collected {} node coordinates ({:.1}MB memory)",
        all_nodes.len(),
        all_nodes.len() as f64 * 16.0 / 1_048_576.0
    );

    eprintln!("Pass 2: Collecting all way geometries...");
    let all_ways = collect_all_ways_with_geometry(input_path, &all_nodes)?;
    eprintln!(
        "Collected {} way geometries ({:.1}MB memory)",
        all_ways.len(),
        all_ways.len() as f64 * 200.0 / 1_048_576.0
    ); // Estimate ~200 bytes per way

    eprintln!("Pass 3: Processing all elements with complete geometry...");
    let reader = ElementReader::from_path(input_path).context("Failed to open PBF file")?;

    // Streaming architecture with complete geometry computation
    let (tx, rx) = mpsc::sync_channel::<String>(1000);
    let tag_filter_clone = tag_filter.clone();
    let all_nodes_clone = all_nodes.clone();
    let all_ways_clone = all_ways;

    // Spawn background thread for immediate output streaming
    let output_thread = {
        let output_path = output_path.cloned();
        thread::spawn(move || -> Result<(), anyhow::Error> {
            // Setup output writer in the output thread
            let mut writer: Box<dyn Write> = match output_path.as_ref() {
                Some(path) => {
                    let file = File::create(path)
                        .with_context(|| format!("Failed to create output file: {}", path))?;
                    Box::new(BufWriter::new(file))
                }
                None => Box::new(std::io::stdout()),
            };

            let mut feature_count = 0usize;

            while let Ok(json_line) = rx.recv() {
                writeln!(writer, "{}", json_line)?; // Stream immediately to output
                feature_count += 1;

                // Memory monitoring every 10k features
                if feature_count % 10000 == 0 {
                    eprintln!("Streamed {} features", feature_count);
                    if let Some(memory_usage) = get_memory_usage_mb() {
                        eprintln!("Current memory usage: {} MB", memory_usage);
                    }
                }
            }

            writer.flush()?;
            eprintln!(
                "Streaming output complete. Total features: {}",
                feature_count
            );
            Ok(())
        })
    };

    // PARALLEL PROCESSING: Complete geometry computation
    reader.par_map_reduce(
        |element| {
            // Parallel map: Process each element on available CPU cores
            let mut results = Vec::new();
            if let Some(osm_element) = process_element(element, &tag_filter_clone) {
                let json_opt = match &osm_element {
                    OsmElement::Node(node) => {
                        if !node.tags.is_empty() {
                            convert_node_to_json(node, pretty_print)
                        } else {
                            None
                        }
                    }
                    OsmElement::Way(way) => {
                        if !way.tags.is_empty() {
                            convert_way_to_json_with_full_geometry(
                                way,
                                &all_nodes_clone,
                                pretty_print,
                            )
                        } else {
                            None
                        }
                    }
                    OsmElement::Relation(relation) => {
                        if !relation.tags.is_empty() {
                            convert_relation_to_json_with_way_resolution(
                                relation,
                                &all_ways_clone,
                                pretty_print,
                            )
                        } else {
                            None
                        }
                    }
                };

                if let Some(json_str) = json_opt
                    && !json_str.is_empty()
                {
                    results.push(json_str);
                }
            }
            results
        },
        Vec::new,
        |mut acc, mut batch| {
            // Reduce: Stream results immediately to output thread
            for json_str in batch.drain(..) {
                if tx.send(json_str).is_err() {
                    return acc; // Output thread disconnected
                }
            }

            // Keep memory bounded - don't accumulate
            acc.clear();
            acc
        },
    )?;

    // Close channel to signal completion
    drop(tx);

    // Wait for output thread to finish
    output_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Output thread panicked"))??;

    Ok(())
}

#[derive(Debug, Clone)]
struct WayGeometry {
    #[allow(dead_code)]
    id: i64,
    coordinates: Vec<(f64, f64)>,
    #[allow(dead_code)]
    centroid: (f64, f64),
    #[allow(dead_code)]
    bounds: Bounds,
}

fn collect_all_ways_with_geometry(
    input_path: &str,
    all_nodes: &HashMap<i64, (f64, f64)>,
) -> Result<HashMap<i64, WayGeometry>> {
    let reader = ElementReader::from_path(input_path)
        .context("Failed to open PBF file for way collection")?;

    let ways = reader.par_map_reduce(
        |element| {
            let mut local_ways = HashMap::new();
            if let Element::Way(way) = element {
                let node_refs: Vec<i64> = way.refs().collect();
                let coordinates: Vec<(f64, f64)> = node_refs
                    .iter()
                    .filter_map(|node_id| all_nodes.get(node_id).cloned())
                    .collect();

                if !coordinates.is_empty() {
                    let centroid = calculate_centroid(&coordinates);
                    let bounds = calculate_bounds(&coordinates);
                    let way_geometry = WayGeometry {
                        id: way.id(),
                        coordinates,
                        centroid,
                        bounds,
                    };
                    local_ways.insert(way.id(), way_geometry);
                }
            }
            local_ways
        },
        HashMap::new,
        |mut acc, batch| {
            acc.extend(batch);
            acc
        },
    )?;

    Ok(ways)
}

fn convert_relation_to_json_with_way_resolution(
    relation: &OsmRelation,
    all_ways: &HashMap<i64, WayGeometry>,
    pretty_print: bool,
) -> Option<String> {
    use serde_json::json;

    // Collect coordinates from all member ways
    let mut all_coordinates = Vec::new();
    for member in &relation.members {
        if member.member_type == MemberType::Way
            && let Some(way_geometry) = all_ways.get(&member.member_id)
        {
            all_coordinates.extend(way_geometry.coordinates.iter().cloned());
        }
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
                "type": "entrance"  // Match GoLang pbf2json format
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
        // Fall back to including members if no geometry available
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

fn convert_way_to_json_with_full_geometry(
    way: &OsmWay,
    all_nodes: &HashMap<i64, (f64, f64)>,
    pretty_print: bool,
) -> Option<String> {
    use serde_json::json;

    // Calculate centroid and bounds from way geometry
    let coordinates: Vec<(f64, f64)> = way
        .node_refs
        .iter()
        .filter_map(|node_id| all_nodes.get(node_id).cloned())
        .collect();

    if coordinates.is_empty() {
        return convert_way_to_json(way, pretty_print);
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

fn convert_relation_to_json_with_full_geometry(
    relation: &OsmRelation,
    all_nodes: &HashMap<i64, (f64, f64)>,
    pretty_print: bool,
) -> Option<String> {
    use serde_json::json;

    // For relations, we need to resolve member ways to compute geometry
    // This is a simplified version - full implementation would need way storage too
    let mut all_coordinates = Vec::new();

    // Collect coordinates from any node members
    for member in &relation.members {
        if member.member_type == MemberType::Node
            && let Some((lat, lon)) = all_nodes.get(&member.member_id)
        {
            all_coordinates.push((*lat, *lon));
        }
    }

    let mut record = json!({
        "id": relation.id,
        "type": "relation",
        "tags": relation.tags
    });

    // If we have coordinates, compute centroid and bounds
    if !all_coordinates.is_empty() {
        let (centroid_lat, centroid_lon) = calculate_centroid(&all_coordinates);
        let bounds = calculate_bounds(&all_coordinates);

        record.as_object_mut().unwrap().insert(
            "centroid".to_string(),
            json!({
                "lat": format!("{:.7}", centroid_lat),
                "lon": format!("{:.7}", centroid_lon),
                "type": "entrance"  // Match GoLang pbf2json format
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
        // Fall back to including members if no geometry available
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
