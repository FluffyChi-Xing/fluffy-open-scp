//! Neutral data model of a `.blend` extraction bundle.
//!
//! `blender_scripts/extract_blend.py` only reads Blender's data model; every
//! export decision lives in Rust.  These types are the contract between the
//! two sides and contain no I/O.

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Bundle {
    pub version: u32,
    pub unit: f64,
    /// Scene-level `ny_facade.frame_color` (window recipe colour source).
    #[serde(default)]
    pub frame_color: Option<[f64; 4]>,
    #[serde(default)]
    pub objects: Vec<BundleObject>,
    #[serde(default)]
    pub materials: Vec<BundleMaterial>,
    #[serde(default)]
    pub node_trees: BTreeMap<String, NodeTree>,
    #[serde(default)]
    pub images: Vec<BundleImage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BundleObject {
    pub name: String,
    #[serde(default)]
    pub props: BTreeMap<String, Value>,
    #[serde(default)]
    pub layers: Vec<BundleLayer>,
    /// Row-major world matrix (16 floats).
    #[serde(default)]
    pub matrix: Vec<f64>,
    pub mesh: Option<BundleMesh>,
}

impl BundleObject {
    pub fn prop_str(&self, key: &str) -> Option<&str> {
        self.props.get(key).and_then(Value::as_str)
    }

    pub fn layer(&self, uid: &str) -> Option<&BundleLayer> {
        self.layers.iter().find(|layer| layer.uid == uid)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BundleLayer {
    pub name: String,
    pub uid: String,
    pub role: String,
    #[serde(default)]
    pub face_ids: String,
    #[serde(default)]
    pub topology: String,
    #[serde(default)]
    pub settings_json: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BundleMesh {
    #[serde(default)]
    pub vertices: Vec<[f64; 3]>,
    #[serde(default)]
    pub polygons: Vec<BundlePolygon>,
    #[serde(default)]
    pub uv_layers: BTreeMap<String, Vec<[f64; 2]>>,
    #[serde(default)]
    pub face_attributes: BTreeMap<String, Vec<Value>>,
    /// Corner-domain attributes (decal coordinates are stored per loop).
    #[serde(default)]
    pub corner_attributes: BTreeMap<String, Vec<[f64; 3]>>,
    #[serde(default)]
    pub materials: Vec<Option<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BundlePolygon {
    pub vertices: Vec<u32>,
    pub loops: Vec<u32>,
    pub normal: [f64; 3],
    pub material: u32,
    #[serde(default)]
    pub area: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BundleMaterial {
    pub name: String,
    #[serde(default)]
    pub props: BTreeMap<String, Value>,
    pub tree: Option<String>,
}

impl BundleMaterial {
    pub fn prop_str(&self, key: &str) -> Option<&str> {
        self.props.get(key).and_then(Value::as_str)
    }

    pub fn prop_f64(&self, key: &str) -> Option<f64> {
        self.props.get(key).and_then(Value::as_f64)
    }

    pub fn prop_bool(&self, key: &str) -> bool {
        self.props
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BundleImage {
    pub name: String,
    #[serde(default)]
    pub size: [u32; 2],
    #[serde(default)]
    pub filepath: String,
    #[serde(default)]
    pub packed: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NodeTree {
    pub name: String,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub links: Vec<Link>,
}

impl NodeTree {
    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.name == name)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Node {
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub inputs: Vec<Socket>,
    #[serde(default)]
    pub outputs: Vec<Socket>,
    pub image: Option<String>,
    pub image_size: Option<[u32; 2]>,
    pub uv_map: Option<String>,
    pub node_tree: Option<String>,
    pub attribute_name: Option<String>,
}

impl Node {
    pub fn input(&self, index: usize) -> Option<&Socket> {
        self.inputs.get(index)
    }

    pub fn input_named(&self, name: &str) -> Option<&Socket> {
        self.inputs.iter().find(|socket| socket.name == name)
    }

    pub fn output(&self, index: usize) -> Option<&Socket> {
        self.outputs.get(index)
    }

    pub fn output_named(&self, name: &str) -> Option<&Socket> {
        self.outputs.iter().find(|socket| socket.name == name)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Socket {
    pub name: String,
    #[serde(default)]
    pub linked: bool,
    #[serde(default)]
    pub value: Value,
}

impl Socket {
    pub fn f64(&self) -> Option<f64> {
        let value = self.value.as_number()?;
        if value.is_f64() {
            // Blender RNA socket floats are f32. Widen the original value
            // exactly so material grouping also matches Python's float keys.
            value.as_f64().map(|value| f64::from(value as f32))
        } else {
            value.as_f64()
        }
    }

    pub fn f32(&self) -> Option<f32> {
        self.f64().map(|value| value as f32)
    }

    pub fn bool(&self) -> Option<bool> {
        self.value.as_bool()
    }

    pub fn string(&self) -> Option<&str> {
        self.value.as_str()
    }

    pub fn array(&self) -> Option<&Vec<Value>> {
        self.value.as_array()
    }

    pub fn vec4(&self) -> Option<[f32; 4]> {
        let values = self.array()?;
        if values.len() < 3 {
            return None;
        }
        let mut result = [0.0; 4];
        for (index, value) in values.iter().take(4).enumerate() {
            result[index] = value.as_f64()? as f32;
        }
        if values.len() == 3 {
            result[3] = 1.0;
        }
        Some(result)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Link {
    pub from_node: String,
    pub from_socket: String,
    pub to_node: String,
    pub to_socket: String,
}

impl Bundle {
    pub fn parse(json: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(json)
    }

    /// The renderable building objects: meshes that are not ground/lot meshes.
    pub fn building_objects(&self) -> impl Iterator<Item = &BundleObject> {
        self.objects.iter().filter(|object| {
            object.mesh.is_some()
                && !object.props.contains_key("ny_lot")
                && !object.props.contains_key("ny_lot_regions")
        })
    }

    pub fn material(&self, name: &str) -> Option<&BundleMaterial> {
        self.materials.iter().find(|material| material.name == name)
    }

    pub fn tree(&self, name: &str) -> Option<&NodeTree> {
        self.node_trees.get(name)
    }
}
