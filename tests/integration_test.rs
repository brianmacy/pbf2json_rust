use pbf2json::*;
use std::collections::HashMap;

#[test]
fn test_osm_element_creation() {
    // Test creating an OSM node
    let mut node_tags = HashMap::new();
    node_tags.insert("amenity".to_string(), "restaurant".to_string());
    node_tags.insert("name".to_string(), "Test Restaurant".to_string());

    let node = OsmNode {
        id: 123456,
        lat: 40.7589,
        lon: -73.9851,
        tags: node_tags,
    };

    assert_eq!(node.id, 123456);
    assert_eq!(node.lat, 40.7589);
    assert_eq!(node.lon, -73.9851);
    assert_eq!(node.tags.get("name").unwrap(), "Test Restaurant");
    assert_eq!(node.tags.get("amenity").unwrap(), "restaurant");

    // Test creating an OSM way
    let mut way_tags = HashMap::new();
    way_tags.insert("highway".to_string(), "residential".to_string());

    let way = OsmWay {
        id: 789012,
        node_refs: vec![1, 2, 3, 4, 5],
        tags: way_tags,
    };

    assert_eq!(way.id, 789012);
    assert_eq!(way.node_refs, vec![1, 2, 3, 4, 5]);
    assert_eq!(way.tags.get("highway").unwrap(), "residential");

    // Test creating an OSM relation
    let mut relation_tags = HashMap::new();
    relation_tags.insert("type".to_string(), "multipolygon".to_string());
    relation_tags.insert("name".to_string(), "Test Park".to_string());

    let members = vec![
        OsmRelationMember {
            member_type: MemberType::Way,
            member_id: 100,
            role: "outer".to_string(),
        },
        OsmRelationMember {
            member_type: MemberType::Way,
            member_id: 200,
            role: "inner".to_string(),
        },
    ];

    let relation = OsmRelation {
        id: 345678,
        members,
        tags: relation_tags,
    };

    assert_eq!(relation.id, 345678);
    assert_eq!(relation.members.len(), 2);
    assert_eq!(relation.members[0].member_type, MemberType::Way);
    assert_eq!(relation.members[0].member_id, 100);
    assert_eq!(relation.members[0].role, "outer");
    assert_eq!(relation.tags.get("type").unwrap(), "multipolygon");
    assert_eq!(relation.tags.get("name").unwrap(), "Test Park");
}

#[test]
fn test_element_filtering() {
    // Create test elements
    let node_element = OsmElement::Node(OsmNode {
        id: 1,
        lat: 0.0,
        lon: 0.0,
        tags: {
            let mut tags = HashMap::new();
            tags.insert("amenity".to_string(), "restaurant".to_string());
            tags
        },
    });

    let way_element = OsmElement::Way(OsmWay {
        id: 2,
        node_refs: vec![1, 2, 3],
        tags: {
            let mut tags = HashMap::new();
            tags.insert("highway".to_string(), "primary".to_string());
            tags
        },
    });

    let relation_element = OsmElement::Relation(OsmRelation {
        id: 3,
        members: vec![],
        tags: {
            let mut tags = HashMap::new();
            tags.insert("type".to_string(), "multipolygon".to_string());
            tags
        },
    });

    // Test single tag filters
    let amenity_filter = vec![vec!["amenity".to_string()]];
    assert!(node_element.matches_filter(&amenity_filter));
    assert!(!way_element.matches_filter(&amenity_filter));
    assert!(!relation_element.matches_filter(&amenity_filter));

    let highway_filter = vec![vec!["highway".to_string()]];
    assert!(!node_element.matches_filter(&highway_filter));
    assert!(way_element.matches_filter(&highway_filter));
    assert!(!relation_element.matches_filter(&highway_filter));

    let type_filter = vec![vec!["type".to_string()]];
    assert!(!node_element.matches_filter(&type_filter));
    assert!(!way_element.matches_filter(&type_filter));
    assert!(relation_element.matches_filter(&type_filter));

    // Test multi tag filter
    let multi_filter = vec![vec!["amenity".to_string()], vec!["highway".to_string()]];
    assert!(node_element.matches_filter(&multi_filter));
    assert!(way_element.matches_filter(&multi_filter));
    assert!(!relation_element.matches_filter(&multi_filter));

    // Test empty filter (should match everything)
    let empty_filter = vec![];
    assert!(node_element.matches_filter(&empty_filter));
    assert!(way_element.matches_filter(&empty_filter));
    assert!(relation_element.matches_filter(&empty_filter));
}
