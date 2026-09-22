use roxmltree::{Document, Node};

const S3_NAMESPACE: &str = "http://s3.amazonaws.com/doc/2006-03-01/";

/// Decode the completion document. Part ordering and ledger checks belong to
/// the completion transaction, not the XML parser.
pub fn parse_complete_multipart(body: &str) -> Result<Vec<(i32, String)>, &'static str> {
    // Default parsing disables DTD entity declarations and external resolution.
    let document = Document::parse(body).map_err(|_| "invalid XML document")?;
    let root = document.root_element();
    if root.tag_name().name() != "CompleteMultipartUpload"
        || !matches!(root.tag_name().namespace(), None | Some(S3_NAMESPACE))
    {
        return Err("expected CompleteMultipartUpload");
    }
    let mut parts = Vec::new();
    for part in root.children().filter(Node::is_element) {
        if part.tag_name().name() != "Part"
            || part.tag_name().namespace() != root.tag_name().namespace()
        {
            return Err("expected Part");
        }
        let number = field(part, "PartNumber")?;
        let etag = field(part, "ETag")?;
        let number = number.trim().parse().map_err(|_| "invalid PartNumber")?;
        parts.push((number, etag.trim().trim_matches('"').to_owned()));
    }
    if parts.is_empty() {
        return Err("the part list is empty");
    }
    Ok(parts)
}

fn field(part: Node<'_, '_>, name: &str) -> Result<String, &'static str> {
    let mut matches = part.children().filter(|node| {
        node.is_element()
            && node.tag_name().name() == name
            && node.tag_name().namespace() == part.tag_name().namespace()
    });
    let node = matches.next().ok_or("missing part field")?;
    if matches.next().is_some() || node.children().any(|child| child.is_element()) {
        return Err("invalid part field");
    }
    Ok(node
        .children()
        .filter(Node::is_text)
        .filter_map(|child| child.text())
        .collect())
}
