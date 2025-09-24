use pbf2json::osm::*;
use std::collections::HashMap;

#[test]
fn test_osm_node_creation() {
    let mut tags = HashMap::new();
    tags.insert("name".to_string(), "Test Node".to_string());
    tags.insert("amenity".to_string(), "restaurant".to_string());

    let node = OsmNode {
        id: 12345,
        lat: 40.7128,
        lon: -74.0060,
        tags,
    };

    assert_eq!(node.id, 12345);
    assert_eq!(node.lat, 40.7128);
    assert_eq!(node.lon, -74.0060);
    assert_eq!(node.tags.get("name"), Some(&"Test Node".to_string()));
    assert_eq!(node.tags.get("amenity"), Some(&"restaurant".to_string()));
}

#[test]
fn test_osm_way_creation() {
    let mut tags = HashMap::new();
    tags.insert("highway".to_string(), "residential".to_string());

    let way = OsmWay {
        id: 67890,
        node_refs: vec![1, 2, 3, 4],
        tags,
    };

    assert_eq!(way.id, 67890);
    assert_eq!(way.node_refs, vec![1, 2, 3, 4]);
    assert_eq!(way.tags.get("highway"), Some(&"residential".to_string()));
}

#[test]
fn test_osm_relation_creation() {
    let mut tags = HashMap::new();
    tags.insert("type".to_string(), "multipolygon".to_string());

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

    assert_eq!(relation.id, 99999);
    assert_eq!(relation.members.len(), 2);
    assert_eq!(relation.members[0].member_id, 123);
    assert_eq!(relation.members[0].role, "outer");
    assert_eq!(relation.members[1].member_id, 456);
    assert_eq!(relation.members[1].role, "inner");
}

#[test]
fn test_osm_element_matches_filter() {
    let mut tags = HashMap::new();
    tags.insert("highway".to_string(), "primary".to_string());
    tags.insert("name".to_string(), "Main Street".to_string());

    let way = OsmWay {
        id: 1,
        node_refs: vec![1, 2, 3],
        tags,
    };

    let element = OsmElement::Way(way);

    assert!(element.matches_filter(&["highway".to_string()]));
    assert!(element.matches_filter(&["name".to_string()]));
    assert!(!element.matches_filter(&["building".to_string()]));
    assert!(element.matches_filter(&["building".to_string(), "highway".to_string()]));
    assert!(element.matches_filter(&[]));
}

#[test]
fn test_is_closed_way() {
    let closed_way = OsmWay {
        id: 1,
        node_refs: vec![1, 2, 3, 4, 1],
        tags: HashMap::new(),
    };

    let open_way = OsmWay {
        id: 2,
        node_refs: vec![1, 2, 3, 4],
        tags: HashMap::new(),
    };

    let empty_way = OsmWay {
        id: 3,
        node_refs: vec![],
        tags: HashMap::new(),
    };

    assert!(is_closed_way(&closed_way));
    assert!(!is_closed_way(&open_way));
    assert!(!is_closed_way(&empty_way));
}

#[test]
fn test_is_area() {
    let mut building_tags = HashMap::new();
    building_tags.insert("building".to_string(), "yes".to_string());

    let building_area = OsmWay {
        id: 1,
        node_refs: vec![1, 2, 3, 4, 1],
        tags: building_tags,
    };

    let mut highway_tags = HashMap::new();
    highway_tags.insert("highway".to_string(), "residential".to_string());

    let highway_line = OsmWay {
        id: 2,
        node_refs: vec![1, 2, 3, 4],
        tags: highway_tags,
    };

    let mut landuse_tags = HashMap::new();
    landuse_tags.insert("landuse".to_string(), "residential".to_string());

    let landuse_area = OsmWay {
        id: 3,
        node_refs: vec![1, 2, 3, 4, 1],
        tags: landuse_tags,
    };

    assert!(is_area(&building_area));
    assert!(!is_area(&highway_line));
    assert!(is_area(&landuse_area));
}
