//! JSON IR wire format for Node ↔ JavaScript transfer.
//!
//! Format: `{"k":"<kind>","v":<value>}` — matches JS IrNode discriminated union.
//! Int64 overflow: `{"k":"int","v64":"<i64_string>"}` (v omitted or null).
//! Float special: `{"k":"float","s":"nan"|"inf"|"-inf"}` (v omitted).
//! Binary: hex-encoded string.
//! Object: `{"k":"object","v":{"key":...}}` preserving insertion order.

use indexmap::IndexMap;

use serde::{Deserialize, Serialize};
use ttoon_core::ir::Node;

/// Max safe integer in JavaScript: 2^53 - 1
const JS_MAX_SAFE_INT: i64 = (1i64 << 53) - 1;
const JS_MIN_SAFE_INT: i64 = -JS_MAX_SAFE_INT;

#[derive(Serialize, Deserialize)]
#[serde(tag = "k")]
pub enum WireNode {
    #[serde(rename = "null")]
    Null,

    #[serde(rename = "bool")]
    Bool { v: bool },

    #[serde(rename = "int")]
    Int {
        #[serde(skip_serializing_if = "Option::is_none")]
        v: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        v64: Option<String>,
    },

    #[serde(rename = "float")]
    Float {
        #[serde(skip_serializing_if = "Option::is_none")]
        v: Option<f64>,
        /// For NaN/Inf/-Inf which can't be represented in JSON.
        #[serde(skip_serializing_if = "Option::is_none")]
        s: Option<String>,
    },

    #[serde(rename = "decimal")]
    Decimal { v: String },

    #[serde(rename = "string")]
    Str { v: String },

    #[serde(rename = "date")]
    Date { v: String },

    #[serde(rename = "time")]
    Time { v: String },

    #[serde(rename = "datetime")]
    DateTime { v: String },

    #[serde(rename = "uuid")]
    Uuid { v: String },

    #[serde(rename = "binary")]
    Binary { v: String },

    #[serde(rename = "list")]
    List { v: Vec<WireNode> },

    #[serde(rename = "object")]
    Object { v: Vec<WireKV> },
}

/// Key-value pair preserving insertion order (IndexMap → array of pairs).
#[derive(Serialize, Deserialize)]
pub struct WireKV {
    pub k: String,
    pub v: WireNode,
}

// ─── Node → WireNode ────────────────────────────────────────────────────────

impl From<&Node> for WireNode {
    fn from(node: &Node) -> Self {
        match node {
            Node::Null => WireNode::Null,
            Node::Bool(b) => WireNode::Bool { v: *b },
            Node::Int(i) => {
                if *i >= JS_MIN_SAFE_INT && *i <= JS_MAX_SAFE_INT {
                    WireNode::Int {
                        v: Some(*i),
                        v64: None,
                    }
                } else {
                    WireNode::Int {
                        v: None,
                        v64: Some(i.to_string()),
                    }
                }
            }
            Node::Float(f) => {
                if f.is_nan() {
                    WireNode::Float {
                        v: None,
                        s: Some("nan".into()),
                    }
                } else if f.is_infinite() {
                    WireNode::Float {
                        v: None,
                        s: Some(if *f > 0.0 { "inf" } else { "-inf" }.into()),
                    }
                } else {
                    WireNode::Float {
                        v: Some(*f),
                        s: None,
                    }
                }
            }
            Node::Decimal(s) => WireNode::Decimal { v: s.clone() },
            Node::String(s) => WireNode::Str { v: s.clone() },
            Node::Date(s) => WireNode::Date { v: s.clone() },
            Node::Time(s) => WireNode::Time { v: s.clone() },
            Node::DateTime(s) => WireNode::DateTime { v: s.clone() },
            Node::Uuid(s) => WireNode::Uuid { v: s.clone() },
            Node::Binary(bytes) => WireNode::Binary {
                v: hex_encode(bytes),
            },
            Node::List(items) => WireNode::List {
                v: items.iter().map(WireNode::from).collect(),
            },
            Node::Object(map) => WireNode::Object {
                v: map
                    .iter()
                    .map(|(k, v)| WireKV {
                        k: k.clone(),
                        v: WireNode::from(v),
                    })
                    .collect(),
            },
        }
    }
}

// ─── WireNode → Node ────────────────────────────────────────────────────────

impl TryFrom<WireNode> for Node {
    type Error = String;

    fn try_from(wire: WireNode) -> Result<Self, Self::Error> {
        match wire {
            WireNode::Null => Ok(Node::Null),
            WireNode::Bool { v } => Ok(Node::Bool(v)),
            WireNode::Int { v, v64 } => {
                if let Some(s) = v64 {
                    let i = s
                        .parse::<i64>()
                        .map_err(|e| format!("invalid int v64 '{}': {}", s, e))?;
                    Ok(Node::Int(i))
                } else if let Some(i) = v {
                    Ok(Node::Int(i))
                } else {
                    Err("int node missing both v and v64".into())
                }
            }
            WireNode::Float { v, s } => {
                if let Some(special) = s {
                    match special.as_str() {
                        "nan" => Ok(Node::Float(f64::NAN)),
                        "inf" => Ok(Node::Float(f64::INFINITY)),
                        "-inf" => Ok(Node::Float(f64::NEG_INFINITY)),
                        _ => Err(format!("unknown float special '{}'", special)),
                    }
                } else if let Some(f) = v {
                    Ok(Node::Float(f))
                } else {
                    Err("float node missing both v and s".into())
                }
            }
            WireNode::Decimal { v } => Ok(Node::Decimal(v)),
            WireNode::Str { v } => Ok(Node::String(v)),
            WireNode::Date { v } => Ok(Node::Date(v)),
            WireNode::Time { v } => Ok(Node::Time(v)),
            WireNode::DateTime { v } => Ok(Node::DateTime(v)),
            WireNode::Uuid { v } => Ok(Node::Uuid(v)),
            WireNode::Binary { v } => {
                let bytes =
                    hex_decode(&v).map_err(|e| format!("invalid binary hex '{}': {}", v, e))?;
                Ok(Node::Binary(bytes))
            }
            WireNode::List { v } => {
                let nodes: Result<Vec<Node>, String> = v.into_iter().map(Node::try_from).collect();
                Ok(Node::List(nodes?))
            }
            WireNode::Object { v } => {
                let mut map = IndexMap::new();
                for kv in v {
                    map.insert(kv.k, Node::try_from(kv.v)?);
                }
                Ok(Node::Object(map))
            }
        }
    }
}

// ─── Hex helpers ─────────────────────────────────────────────────────────────

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("odd-length hex string".into());
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex[i..i + 2], 16)
            .map_err(|e| format!("invalid hex at {}: {}", i, e))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

// ─── Public API ──────────────────────────────────────────────────────────────

pub fn node_to_json(node: &Node) -> Result<String, String> {
    let wire = WireNode::from(node);
    serde_json::to_string(&wire).map_err(|e| format!("JSON serialize: {}", e))
}

pub fn json_to_node(json: &str) -> Result<Node, String> {
    let wire: WireNode =
        serde_json::from_str(json).map_err(|e| format!("JSON deserialize: {}", e))?;
    Node::try_from(wire)
}
