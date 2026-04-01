use super::ir::Node;
use super::typed_fmt;
use super::{Result, TjsonOptions};

pub fn serialize_tjson(node: &Node, opts: &TjsonOptions) -> Result<String> {
    let mut buffer = String::with_capacity(256);
    serialize_structure_to_buffer(node, opts, &mut buffer)?;
    Ok(buffer)
}

pub(crate) fn format_scalar_to_tjson_buffer(
    buffer: &mut String,
    node: &Node,
    opts: &TjsonOptions,
) -> Result<()> {
    match node {
        Node::Null => buffer.push_str("null"),
        Node::Bool(value) => buffer.push_str(if *value { "true" } else { "false" }),
        Node::Int(value) => typed_fmt::fmt_int(buffer, *value),
        Node::Float(value) => typed_fmt::fmt_float(buffer, *value),
        Node::Decimal(value) => buffer.push_str(value),
        Node::String(value) => typed_fmt::fmt_tjson_string(buffer, value),
        Node::Date(value) => buffer.push_str(value),
        Node::Time(value) => buffer.push_str(value),
        Node::DateTime(value) => typed_fmt::fmt_datetime(buffer, value),
        Node::Uuid(value) => typed_fmt::fmt_uuid(buffer, value),
        Node::Binary(value) => typed_fmt::fmt_binary(buffer, value, opts.binary_format)?,
        Node::List(_) | Node::Object(_) => {
            return Err(super::Error::new(
                super::ErrorKind::SerializeError,
                "stream_write_tjson: nested structures are not allowed in tabular writer",
                None,
            ))
        }
    }
    Ok(())
}

fn serialize_structure_to_buffer(
    node: &Node,
    opts: &TjsonOptions,
    buffer: &mut String,
) -> Result<()> {
    match node {
        Node::Null
        | Node::Bool(_)
        | Node::Int(_)
        | Node::Float(_)
        | Node::Decimal(_)
        | Node::String(_)
        | Node::Date(_)
        | Node::Time(_)
        | Node::DateTime(_)
        | Node::Uuid(_)
        | Node::Binary(_) => format_scalar_to_tjson_buffer(buffer, node, opts)?,
        Node::List(items) => serialize_list_to_buffer(items, opts, buffer)?,
        Node::Object(items) => serialize_object_to_buffer(items, opts, buffer)?,
    }
    Ok(())
}

fn serialize_list_to_buffer(
    items: &[Node],
    opts: &TjsonOptions,
    buffer: &mut String,
) -> Result<()> {
    buffer.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            buffer.push_str(", ");
        }
        serialize_structure_to_buffer(item, opts, buffer)?;
    }
    buffer.push(']');
    Ok(())
}

fn serialize_object_to_buffer(
    items: &indexmap::IndexMap<String, Node>,
    opts: &TjsonOptions,
    buffer: &mut String,
) -> Result<()> {
    buffer.push('{');
    for (i, (key, value)) in items.iter().enumerate() {
        if i > 0 {
            buffer.push_str(", ");
        }
        buffer.push('"');
        typed_fmt::escape_tjson_string(buffer, key);
        buffer.push_str("\": ");
        serialize_structure_to_buffer(value, opts, buffer)?;
    }
    buffer.push('}');
    Ok(())
}
