//! Classify supported requests before any object mutation.

#[derive(Debug, PartialEq, Eq)]
pub enum Operation<'a> {
    Put,
    Get,
    Head,
    Delete,
    CreateMultipart,
    UploadPart {
        upload_id: &'a str,
        part_number: i32,
    },
    CompleteMultipart {
        upload_id: &'a str,
    },
    AbortMultipart {
        upload_id: &'a str,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum RouteError {
    InvalidArgument,
    NotImplemented,
    MethodNotAllowed,
}

pub fn classify<'a>(
    method: &str,
    query: &'a str,
    copy_source: bool,
) -> Result<Operation<'a>, RouteError> {
    if copy_source
        || [
            "acl",
            "tagging",
            "versionId",
            "torrent",
            "restore",
            "retention",
            "legal-hold",
            "attributes",
            "select",
            "select-type",
        ]
        .iter()
        .any(|key| query_flag(query, key))
    {
        return Err(RouteError::NotImplemented);
    }
    let uploads = query_flag(query, "uploads");
    let has_id = query_flag(query, "uploadId");
    let has_part = query_flag(query, "partNumber");
    for key in ["uploads", "uploadId", "partNumber"] {
        if query
            .split('&')
            .filter(|pair| pair.split('=').next() == Some(key))
            .count()
            > 1
        {
            return Err(RouteError::InvalidArgument);
        }
    }
    if uploads {
        return match (method, has_id, has_part) {
            ("POST", false, false) => Ok(Operation::CreateMultipart),
            (_, true, _) | (_, _, true) => Err(RouteError::InvalidArgument),
            _ => Err(RouteError::NotImplemented),
        };
    }
    if has_id {
        let id = query_value(query, "uploadId")
            .filter(|id| !id.is_empty())
            .ok_or(RouteError::InvalidArgument)?;
        return match method {
            "PUT" => {
                let number = query_value(query, "partNumber")
                    .and_then(|v| v.parse::<i32>().ok())
                    .filter(|n| (1..=10000).contains(n))
                    .ok_or(RouteError::InvalidArgument)?;
                Ok(Operation::UploadPart {
                    upload_id: id,
                    part_number: number,
                })
            }
            "POST" if !has_part => Ok(Operation::CompleteMultipart { upload_id: id }),
            "DELETE" if !has_part => Ok(Operation::AbortMultipart { upload_id: id }),
            "POST" | "DELETE" => Err(RouteError::InvalidArgument),
            _ => Err(RouteError::NotImplemented),
        };
    }
    if has_part {
        return Err(if method == "PUT" {
            RouteError::InvalidArgument
        } else {
            RouteError::NotImplemented
        });
    }
    match method {
        "PUT" => Ok(Operation::Put),
        "GET" => Ok(Operation::Get),
        "HEAD" => Ok(Operation::Head),
        "DELETE" => Ok(Operation::Delete),
        _ => Err(RouteError::MethodNotAllowed),
    }
}

pub fn query_flag(query: &str, key: &str) -> bool {
    query
        .split('&')
        .any(|pair| pair == key || pair.split_once('=').is_some_and(|(k, _)| k == key))
}

/// Keep the original encoded value until signature verification is complete.
pub fn query_value<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, value)| value)
}
