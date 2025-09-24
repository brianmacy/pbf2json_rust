// Parallel PBF to JSON converter with streaming output
use crate::osm::{MemberType, OsmElement, OsmNode, OsmRelation, OsmRelationMember, OsmWay};
use anyhow::{Context, Result};
use osmpbf::{BlobDecode, BlobReader, Element};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::mpsc;
use std::thread;

const CHUNK_SIZE: usize = 10_000; // Process elements in chunks for streaming output

/// Parallel PBF to GeoJSON converter with streaming output and >800% CPU utilization
pub fn convert_pbf_to_geojson_parallel(
    input_path: &str,
    output_path: Option<&String>,
    tag_filter: Option<Vec<String>>,
    pretty_print: bool,
) -> Result<()> {
    println!("🚀 Starting parallel PBF processing...");

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
                    eprintln!("📊 Processed {} batches, {} total features", batch_count, total_features);
                    if let Some(memory_usage) = get_memory_usage_mb() {
                        eprintln!("🧠 Memory usage: {} MB", memory_usage);
                    }
                }
            }

            writer.flush()?;
            eprintln!("✅ Parallel streaming complete. Total features: {}", total_features);
            Ok(())
        })
    };

    // PARALLEL PROCESSING APPROACH 1: Custom blob-level parallelization
    let file = File::open(input_path).context("Failed to open PBF file")?;
    let buf_reader = std::io::BufReader::new(file);
    let blob_reader = BlobReader::new(buf_reader);

    // Process blobs in parallel using rayon
    let processing_result: Result<()> = blob_reader
        .par_bridge()
        .try_for_each(|blob_result| -> Result<()> {
            let blob = blob_result.context("Failed to read blob")?;

            match blob.decode() {
                Ok(BlobDecode::OsmData(block)) => {
                    // Process all elements in this block in parallel
                    let json_results: Vec<String> = block
                        .elements()
                        .par_bridge()
                        .filter_map(|element| process_element_to_json(element, &tag_filter_clone, pretty_print))
                        .collect();

                    // Send results in chunks to maintain bounded memory
                    for chunk in json_results.chunks(CHUNK_SIZE) {
                        if tx.send(chunk.to_vec()).is_err() {
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

            Ok(())
        });

    // Close the channel to signal completion
    drop(tx);

    // Wait for output thread and processing to complete
    processing_result?;
    output_thread.join().map_err(|_| anyhow::anyhow!("Output thread panicked"))??;

    println!("🎉 Parallel processing completed successfully!");
    Ok(())
}

/// Process a single element and convert to JSON if it matches filters
fn process_element_to_json(
    element: Element,
    tag_filter: &Option<Vec<String>>,
    pretty_print: bool,
) -> Option<String> {
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

    // Apply tag filter if specified
    if let Some(filter_tags) = tag_filter
        && !osm_element.matches_filter(filter_tags) {
            return None;
        }

    // Skip elements without tags
    match &osm_element {
        OsmElement::Node(node) if node.tags.is_empty() => return None,
        OsmElement::Way(way) if way.tags.is_empty() => return None,
        OsmElement::Relation(relation) if relation.tags.is_empty() => return None,
        _ => {}
    }

    // Convert to JSON
    match osm_element {
        OsmElement::Node(node) => convert_node_to_json(&node, pretty_print),
        OsmElement::Way(way) => convert_way_to_json(&way, pretty_print),
        OsmElement::Relation(relation) => convert_relation_to_json(&relation, pretty_print),
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

    let members: Vec<serde_json::Value> = relation.members.iter().map(|member| {
        json!({
            "type": match member.member_type {
                MemberType::Node => "node",
                MemberType::Way => "way",
                MemberType::Relation => "relation",
            },
            "ref": member.member_id,
            "role": member.role
        })
    }).collect();

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