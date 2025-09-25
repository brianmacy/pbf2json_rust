use pbf2json::*;
use std::collections::HashMap;

#[test]
fn test_exact_tag_matching() {
    let mut tags = HashMap::new();
    tags.insert("amenity".to_string(), "restaurant".to_string());
    tags.insert("name".to_string(), "Pizza Place".to_string());

    let element = OsmElement::Node(OsmNode {
        id: 1,
        lat: 0.0,
        lon: 0.0,
        tags,
    });

    // Test exact match
    let filter = vec![vec!["amenity".to_string()]];
    assert!(element.matches_filter(&filter));

    // Test non-matching exact tag
    let filter = vec![vec!["highway".to_string()]];
    assert!(!element.matches_filter(&filter));
}

#[test]
fn test_or_logic_comma_separated() {
    let mut tags = HashMap::new();
    tags.insert("amenity".to_string(), "restaurant".to_string());

    let element = OsmElement::Node(OsmNode {
        id: 1,
        lat: 0.0,
        lon: 0.0,
        tags,
    });

    // Test OR logic: amenity OR highway (should match because has amenity)
    let filter = vec![vec!["amenity".to_string()], vec!["highway".to_string()]];
    assert!(element.matches_filter(&filter));

    // Test OR logic: highway OR building (should not match)
    let filter = vec![vec!["highway".to_string()], vec!["building".to_string()]];
    assert!(!element.matches_filter(&filter));
}

#[test]
fn test_and_logic_plus_separated() {
    let mut tags = HashMap::new();
    tags.insert("amenity".to_string(), "restaurant".to_string());
    tags.insert("name".to_string(), "Pizza Place".to_string());
    tags.insert("cuisine".to_string(), "italian".to_string());

    let element = OsmElement::Node(OsmNode {
        id: 1,
        lat: 0.0,
        lon: 0.0,
        tags,
    });

    // Test AND logic: amenity AND name (should match)
    let filter = vec![vec!["amenity".to_string(), "name".to_string()]];
    assert!(element.matches_filter(&filter));

    // Test AND logic: amenity AND highway (should not match - missing highway)
    let filter = vec![vec!["amenity".to_string(), "highway".to_string()]];
    assert!(!element.matches_filter(&filter));

    // Test AND logic: amenity AND name AND cuisine (should match - has all three)
    let filter = vec![vec![
        "amenity".to_string(),
        "name".to_string(),
        "cuisine".to_string(),
    ]];
    assert!(element.matches_filter(&filter));
}

#[test]
fn test_combined_and_or_logic() {
    let mut restaurant_tags = HashMap::new();
    restaurant_tags.insert("amenity".to_string(), "restaurant".to_string());
    restaurant_tags.insert("name".to_string(), "Pizza Place".to_string());

    let restaurant = OsmElement::Node(OsmNode {
        id: 1,
        lat: 0.0,
        lon: 0.0,
        tags: restaurant_tags,
    });

    let mut road_tags = HashMap::new();
    road_tags.insert("highway".to_string(), "residential".to_string());
    road_tags.insert("name".to_string(), "Main Street".to_string());

    let road = OsmElement::Way(OsmWay {
        id: 2,
        node_refs: vec![1, 2, 3],
        tags: road_tags,
    });

    // Test: (amenity AND name) OR (highway AND name)
    // Both restaurant and road should match
    let filter = vec![
        vec!["amenity".to_string(), "name".to_string()], // AND group 1
        vec!["highway".to_string(), "name".to_string()], // AND group 2
    ];
    assert!(restaurant.matches_filter(&filter));
    assert!(road.matches_filter(&filter));

    // Test: (amenity AND cuisine) OR (highway AND surface)
    // Neither should match (missing required tags)
    let filter = vec![
        vec!["amenity".to_string(), "cuisine".to_string()],
        vec!["highway".to_string(), "surface".to_string()],
    ];
    assert!(!restaurant.matches_filter(&filter));
    assert!(!road.matches_filter(&filter));
}

#[test]
fn test_wildcard_all_tags() {
    let mut tags = HashMap::new();
    tags.insert("amenity".to_string(), "restaurant".to_string());

    let element_with_tags = OsmElement::Node(OsmNode {
        id: 1,
        lat: 0.0,
        lon: 0.0,
        tags,
    });

    let element_no_tags = OsmElement::Node(OsmNode {
        id: 2,
        lat: 0.0,
        lon: 0.0,
        tags: HashMap::new(),
    });

    // Test '*' wildcard (matches any element with tags)
    let filter = vec![vec!["*".to_string()]];
    assert!(element_with_tags.matches_filter(&filter));
    assert!(!element_no_tags.matches_filter(&filter));
}

#[test]
fn test_wildcard_prefix_matching() {
    let mut tags = HashMap::new();
    tags.insert("addr:street".to_string(), "Main St".to_string());
    tags.insert("addr:housenumber".to_string(), "123".to_string());
    tags.insert("addr:city".to_string(), "Springfield".to_string());
    tags.insert("name".to_string(), "Test Place".to_string());

    let element = OsmElement::Node(OsmNode {
        id: 1,
        lat: 0.0,
        lon: 0.0,
        tags,
    });

    // Test prefix wildcard 'addr*' (should match all addr: tags)
    let filter = vec![vec!["addr*".to_string()]];
    assert!(element.matches_filter(&filter));

    // Test prefix wildcard 'highway*' (should not match)
    let filter = vec![vec!["highway*".to_string()]];
    assert!(!element.matches_filter(&filter));

    // Test prefix wildcard 'name*' (should match name)
    let filter = vec![vec!["name*".to_string()]];
    assert!(element.matches_filter(&filter));
}

#[test]
fn test_wildcard_suffix_matching() {
    let mut tags = HashMap::new();
    tags.insert("name:en".to_string(), "Test Place".to_string());
    tags.insert("name:fr".to_string(), "Place de Test".to_string());
    tags.insert("addr:street:zh".to_string(), "测试街".to_string());
    tags.insert("highway".to_string(), "residential".to_string());

    let element = OsmElement::Node(OsmNode {
        id: 1,
        lat: 0.0,
        lon: 0.0,
        tags,
    });

    // Test suffix wildcard '*:en' (should match name:en)
    let filter = vec![vec!["*:en".to_string()]];
    assert!(element.matches_filter(&filter));

    // Test suffix wildcard '*:zh' (should match addr:street:zh)
    let filter = vec![vec!["*:zh".to_string()]];
    assert!(element.matches_filter(&filter));

    // Test suffix wildcard '*:de' (should not match)
    let filter = vec![vec!["*:de".to_string()]];
    assert!(!element.matches_filter(&filter));
}

#[test]
fn test_complex_wildcard_and_logic() {
    let mut tags = HashMap::new();
    tags.insert("addr:street".to_string(), "Main St".to_string());
    tags.insert("addr:housenumber".to_string(), "123".to_string());
    tags.insert("name:en".to_string(), "Test Place".to_string());
    tags.insert("amenity".to_string(), "restaurant".to_string());

    let element = OsmElement::Node(OsmNode {
        id: 1,
        lat: 0.0,
        lon: 0.0,
        tags,
    });

    // Test: addr* AND name* (should match - has both addr:street and name:en)
    let filter = vec![vec!["addr*".to_string(), "name*".to_string()]];
    assert!(element.matches_filter(&filter));

    // Test: addr* AND highway* (should not match - missing highway tags)
    let filter = vec![vec!["addr*".to_string(), "highway*".to_string()]];
    assert!(!element.matches_filter(&filter));

    // Test: (addr* AND amenity) OR (highway AND name)
    let filter = vec![
        vec!["addr*".to_string(), "amenity".to_string()],
        vec!["highway".to_string(), "name".to_string()],
    ];
    assert!(element.matches_filter(&filter));
}

#[test]
fn test_real_world_address_filtering() {
    let mut address_node_tags = HashMap::new();
    address_node_tags.insert("addr:street".to_string(), "Main Street".to_string());
    address_node_tags.insert("addr:housenumber".to_string(), "123".to_string());
    address_node_tags.insert("addr:city".to_string(), "Springfield".to_string());

    let address_node = OsmElement::Node(OsmNode {
        id: 1,
        lat: 40.7128,
        lon: -74.0060,
        tags: address_node_tags,
    });

    // Original pbf2json syntax: "addr:housenumber,addr:street" (OR logic)
    let filter = vec![
        vec!["addr:housenumber".to_string()],
        vec!["addr:street".to_string()],
    ];
    assert!(address_node.matches_filter(&filter));

    // New enhanced syntax: "addr:street+addr:housenumber" (AND logic)
    let filter = vec![vec![
        "addr:street".to_string(),
        "addr:housenumber".to_string(),
    ]];
    assert!(address_node.matches_filter(&filter));

    // New wildcard syntax: "addr*" (prefix matching)
    let filter = vec![vec!["addr*".to_string()]];
    assert!(address_node.matches_filter(&filter));

    // Complex example: "addr*+name,highway"
    // Means: (any addr tag AND name) OR highway
    let filter = vec![
        vec!["addr*".to_string(), "name".to_string()], // Should not match (no name)
        vec!["highway".to_string()],                   // Should not match (no highway)
    ];
    assert!(!address_node.matches_filter(&filter));
}

#[test]
fn test_empty_filter() {
    let mut tags = HashMap::new();
    tags.insert("amenity".to_string(), "restaurant".to_string());

    let element = OsmElement::Node(OsmNode {
        id: 1,
        lat: 0.0,
        lon: 0.0,
        tags,
    });

    // Empty filter should match everything
    let filter = vec![];
    assert!(element.matches_filter(&filter));
}

#[test]
fn test_matches_tag_pattern_edge_cases() {
    let mut tags = HashMap::new();
    tags.insert("addr:street:en".to_string(), "Main Street".to_string());
    tags.insert("a".to_string(), "short".to_string());
    tags.insert("very:long:tag:name:here".to_string(), "value".to_string());

    let element = OsmElement::Node(OsmNode {
        id: 1,
        lat: 0.0,
        lon: 0.0,
        tags,
    });

    // Test middle wildcard: "addr:*:en"
    assert!(element.matches_tag_pattern("addr:*:en"));
    assert!(!element.matches_tag_pattern("addr:*:fr"));

    // Test single character tag
    assert!(element.matches_tag_pattern("a"));
    assert!(element.matches_tag_pattern("a*"));

    // Test very long tag
    assert!(element.matches_tag_pattern("very:long:tag:name:here"));
    assert!(element.matches_tag_pattern("very*"));
    assert!(element.matches_tag_pattern("*:here"));
}
