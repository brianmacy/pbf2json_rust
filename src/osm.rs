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

    pub fn matches_filter(&self, filter_tags: &[String]) -> bool {
        if filter_tags.is_empty() {
            return true;
        }

        filter_tags.iter().any(|tag| self.has_tag(tag))
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
