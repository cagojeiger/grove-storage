use grove_object_policy::multipart::composite_etag;

#[test]
fn etag_decodes_hex_case_and_preserves_part_order() {
    let first = "0123456789abcdef0123456789abcdef";
    let second = "00000000000000000000000000000000";
    assert_eq!(
        composite_etag([first, second]),
        composite_etag([first.to_uppercase().as_str(), second])
    );
    assert_ne!(
        composite_etag([first, second]),
        composite_etag([second, first])
    );
}

#[test]
fn composite_etag_matches_known_vectors() {
    let zero_md5 = "00000000000000000000000000000000";
    assert_eq!(
        composite_etag([zero_md5, zero_md5]),
        "70bc8f4b72a86921468bf8e8441dce51-2"
    );
    assert_eq!(
        composite_etag([zero_md5]),
        "4ae71336e44bf9bf79d2752e234818a5-1"
    );
}
