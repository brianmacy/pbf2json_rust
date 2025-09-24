use pbf2json::*;
use serde_json::Value;
use std::collections::HashMap;

#[test]
fn test_element_matches_filter() {
    let mut highway_tags = HashMap::new();
    highway_tags.insert("highway".to_string(), "primary".to_string());

    let highway_element = OsmElement::Way(OsmWay {
        id: 1,
        node_refs: vec![1, 2, 3],
        tags: highway_tags,
    });

    let mut building_tags = HashMap::new();
    building_tags.insert("building".to_string(), "yes".to_string());

    let building_element = OsmElement::Way(OsmWay {
        id: 2,
        node_refs: vec![4, 5, 6],
        tags: building_tags,
    });

    let highway_filter = vec!["highway".to_string()];
    assert!(highway_element.matches_filter(&highway_filter));
    assert!(!building_element.matches_filter(&highway_filter));

    let building_filter = vec!["building".to_string()];
    assert!(!highway_element.matches_filter(&building_filter));
    assert!(building_element.matches_filter(&building_filter));

    let multi_filter = vec!["highway".to_string(), "building".to_string()];
    assert!(highway_element.matches_filter(&multi_filter));
    assert!(building_element.matches_filter(&multi_filter));

    let empty_filter = vec![];
    assert!(highway_element.matches_filter(&empty_filter));
    assert!(building_element.matches_filter(&empty_filter));
}

#[test]
fn test_osm_node_json_conversion() {
    let mut tags = HashMap::new();
    tags.insert("name".to_string(), "Test Node".to_string());
    tags.insert("amenity".to_string(), "restaurant".to_string());

    let node = OsmNode {
        id: 12345,
        lat: 40.7128,
        lon: -74.0060,
        tags,
    };

    // Test that our JSON serialization works
    let json_str = serde_json::to_string(&node).expect("Should serialize to JSON");
    let parsed: Value = serde_json::from_str(&json_str).expect("Should parse JSON");

    assert_eq!(parsed["id"], 12345);
    assert_eq!(parsed["lat"], 40.7128);
    assert_eq!(parsed["lon"], -74.0060);
    assert_eq!(parsed["tags"]["name"], "Test Node");
    assert_eq!(parsed["tags"]["amenity"], "restaurant");
}

#[test]
fn test_osm_way_json_conversion() {
    let mut tags = HashMap::new();
    tags.insert("highway".to_string(), "residential".to_string());

    let way = OsmWay {
        id: 67890,
        node_refs: vec![1, 2, 3, 4],
        tags,
    };

    // Test that our JSON serialization works
    let json_str = serde_json::to_string(&way).expect("Should serialize to JSON");
    let parsed: Value = serde_json::from_str(&json_str).expect("Should parse JSON");

    assert_eq!(parsed["id"], 67890);
    assert_eq!(parsed["node_refs"], serde_json::json!([1, 2, 3, 4]));
    assert_eq!(parsed["tags"]["highway"], "residential");
}

#[test]
fn test_osm_relation_json_conversion() {
    let mut tags = HashMap::new();
    tags.insert("type".to_string(), "multipolygon".to_string());
    tags.insert("name".to_string(), "Test Area".to_string());

    let members = vec![
        OsmRelationMember {
            member_type: MemberType::Way,
            member_id: 123,
            role: "outer".to_string(),
        },
        OsmRelationMember {
            member_type: MemberType::Way,
            member_id: 456,
            role: "inner".to_string(),
        },
    ];

    let relation = OsmRelation {
        id: 99999,
        members,
        tags,
    };

    // Test that our JSON serialization works
    let json_str = serde_json::to_string(&relation).expect("Should serialize to JSON");
    let parsed: Value = serde_json::from_str(&json_str).expect("Should parse JSON");

    assert_eq!(parsed["id"], 99999);
    assert_eq!(parsed["tags"]["type"], "multipolygon");
    assert_eq!(parsed["tags"]["name"], "Test Area");
    assert_eq!(parsed["members"][0]["member_id"], 123);
    assert_eq!(parsed["members"][0]["role"], "outer");
    assert_eq!(parsed["members"][1]["member_id"], 456);
    assert_eq!(parsed["members"][1]["role"], "inner");
}