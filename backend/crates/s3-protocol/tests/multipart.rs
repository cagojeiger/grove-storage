use grove_s3_protocol::multipart::parse_complete_multipart as parse;

#[test]
fn supports_namespaces_entities_and_cdata() {
    for body in [
        "<CompleteMultipartUpload xmlns='http://s3.amazonaws.com/doc/2006-03-01/'><Part><ETag>&quot;a&#98;c&quot;</ETag><PartNumber>1</PartNumber></Part></CompleteMultipartUpload>",
        "<s:CompleteMultipartUpload xmlns:s='http://s3.amazonaws.com/doc/2006-03-01/'><s:Part><s:PartNumber>1</s:PartNumber><s:ETag><![CDATA[\"abc\"]]></s:ETag></s:Part></s:CompleteMultipartUpload>",
    ] {
        assert_eq!(parse(body), Ok(vec![(1, "abc".to_owned())]));
    }
}

#[test]
fn rejects_truncated_wrong_root_and_duplicate_fields() {
    for body in [
        "<CompleteMultipartUpload><Part><PartNumber>1</PartNumber><ETag>x</ETag>",
        "<Wrong><Part><PartNumber>1</PartNumber><ETag>x</ETag></Part></Wrong>",
        "<CompleteMultipartUpload><Part><PartNumber>1</PartNumber><PartNumber>2</PartNumber><ETag>x</ETag></Part></CompleteMultipartUpload>",
        "<CompleteMultipartUpload><Part><PartNumber>1</PartNumber><ETag><nested>x</nested></ETag></Part></CompleteMultipartUpload>",
        "<CompleteMultipartUpload xmlns='urn:other'><Part><PartNumber>1</PartNumber><ETag>x</ETag></Part></CompleteMultipartUpload>",
        "<CompleteMultipartUpload><Part><PartNumber>1</PartNumber><ETag>&undefined;</ETag></Part></CompleteMultipartUpload>",
    ] {
        assert!(parse(body).is_err(), "accepted {body}");
    }
}

#[test]
fn rejects_entity_declarations() {
    let body = "<!DOCTYPE CompleteMultipartUpload [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><CompleteMultipartUpload><Part><PartNumber>1</PartNumber><ETag>&x;</ETag></Part></CompleteMultipartUpload>";
    assert!(parse(body).is_err());
}

#[test]
fn does_not_decode_entities_twice() {
    let body = "<CompleteMultipartUpload><Part><PartNumber>1</PartNumber><ETag>&amp;quot;x&amp;quot;</ETag></Part></CompleteMultipartUpload>";
    assert_eq!(parse(body), Ok(vec![(1, "&quot;x&quot;".to_owned())]));
}
