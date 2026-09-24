use grove_s3_protocol::operation::{Operation, RouteError, classify};

#[test]
fn routes_the_eight_supported_operations() {
    for (method, query, expected) in [
        ("PUT", "", Operation::Put),
        ("GET", "", Operation::Get),
        ("HEAD", "", Operation::Head),
        ("DELETE", "", Operation::Delete),
        ("POST", "uploads", Operation::CreateMultipart),
        (
            "PUT",
            "uploadId=id&partNumber=1",
            Operation::UploadPart {
                upload_id: "id",
                part_number: 1,
            },
        ),
        (
            "POST",
            "uploadId=id",
            Operation::CompleteMultipart { upload_id: "id" },
        ),
        (
            "DELETE",
            "uploadId=id",
            Operation::AbortMultipart { upload_id: "id" },
        ),
    ] {
        assert_eq!(classify(method, query, false), Ok(expected));
    }
}

#[test]
fn unsupported_operations_never_fall_through_to_object_io() {
    assert_eq!(classify("PUT", "", true), Err(RouteError::NotImplemented));
    for (method, query) in [
        ("GET", "uploadId=id"),
        ("DELETE", "tagging"),
        ("PUT", "acl"),
        ("GET", "partNumber=1"),
        ("DELETE", "versionId=v"),
    ] {
        assert_eq!(
            classify(method, query, false),
            Err(RouteError::NotImplemented)
        );
    }
}

#[test]
fn incomplete_or_ambiguous_multipart_is_rejected() {
    for (method, query) in [
        ("PUT", "uploadId=id"),
        ("PUT", "partNumber=1"),
        ("POST", "uploads&uploadId=id"),
        ("DELETE", "uploadId="),
        ("POST", "uploadId=id&uploadId=other"),
        ("PUT", "uploadId=id&partNumber=10001"),
    ] {
        assert_eq!(
            classify(method, query, false),
            Err(RouteError::InvalidArgument)
        );
    }
}

#[test]
fn signing_and_response_parameters_preserve_object_routing() {
    assert_eq!(
        classify(
            "GET",
            "X-Amz-Signature=x&response-content-type=text%2Fplain",
            false
        ),
        Ok(Operation::Get)
    );
}
