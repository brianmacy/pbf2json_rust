use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsmNode {
    pub id: i64,
    pub lat: f64,
    pub lon: f64,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsmWay {
    pub id: i64,
    pub node_refs: Vec<i64>,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsmRelationMember {
    pub member_type: MemberType,
    pub member_id: i64,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemberType {
    Node,
    Way,
    Relation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsmRelation {
    pub id: i64,
    pub members: Vec<OsmRelationMember>,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum OsmElement {
    Node(OsmNode),
    Way(OsmWay),
    Relation(OsmRelation),
}

impl OsmElement {
    #[allow(dead_code)]
    pub fn id(&self) -> i64 {
        match self {
            OsmElement::Node(node) => node.id,
            OsmElement::Way(way) => way.id,
            OsmElement::Relation(relation) => relation.id,
        }
    }

    pub fn tags(&self) -> &HashMap<String, String> {
        match self {
            OsmElement::Node(node) => &node.tags,
            OsmElement::Way(way) => &way.tags,
            OsmElement::Relation(relation) => &relation.tags,
        }
    }

    pub fn has_tag(&self, key: &str) -> bool {
        self.tags().contains_key(key)
    }

    #[allow(dead_code)]
    pub fn get_tag(&self, key: &str) -> Option<&String> {
        self.tags().get(key)
    }

    pub fn matches_filter(&self, filter_tags: &[Vec<String>]) -> bool {
        if filter_tags.is_empty() {
            return true;
        }

        // OR logic between groups: any group that matches makes the element match
        filter_tags.iter().any(|and_group| {
            // AND logic within group: all tags in the group must match
            and_group
                .iter()
                .all(|tag_pattern| self.matches_tag_pattern(tag_pattern))
        })
    }

    /// Check if element matches a tag pattern (supports wildcards with *)
    pub fn matches_tag_pattern(&self, pattern: &str) -> bool {
        if pattern == "*" {
            // Special case: '*' matches any element that has at least one tag
            return !self.tags().is_empty();
        }

        if let Some(prefix) = pattern.strip_suffix('*') {
            // Prefix wildcard: "addr*" matches "addr:street", "addr:housenumber", etc.
            return self.tags().keys().any(|key| key.starts_with(prefix));
        }

        if let Some(suffix) = pattern.strip_prefix('*') {
            // Suffix wildcard: "*:en" matches "name:en", "addr:street:en", etc.
            return self.tags().keys().any(|key| key.ends_with(suffix));
        }

        if pattern.contains('*') {
            // Middle wildcard: "addr:*:en" matches "addr:street:en", etc.
            let parts: Vec<&str> = pattern.split('*').collect();
            return self.tags().keys().any(|key| {
                let mut key_pos = 0;
                for (i, part) in parts.iter().enumerate() {
                    if part.is_empty() {
                        continue;
                    }
                    if let Some(found_pos) = key[key_pos..].find(part) {
                        key_pos += found_pos + part.len();
                        // For the last part, it must be at the end (unless it's empty)
                        if i == parts.len() - 1
                            && key_pos != key.len()
                            && !parts.last().unwrap().is_empty()
                        {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                true
            });
        }

        // Exact match: no wildcards
        self.has_tag(pattern)
    }
}

#[allow(dead_code)]
pub fn is_closed_way(way: &OsmWay) -> bool {
    !way.node_refs.is_empty() && way.node_refs.first() == way.node_refs.last()
}

#[allow(dead_code)]
pub fn is_area(way: &OsmWay) -> bool {
    if !is_closed_way(way) {
        return false;
    }

    way.tags.contains_key("area")
        || way.tags.contains_key("building")
        || way.tags.contains_key("landuse")
        || way.tags.contains_key("leisure")
        || way.tags.contains_key("natural")
        || way.tags.contains_key("amenity")
        || way
            .tags
            .get("highway")
            .is_some_and(|v| v == "pedestrian" || v == "service")
}
