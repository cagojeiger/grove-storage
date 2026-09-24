use grove_s3_protocol::signing::{canonicalize_query, sign};

#[test]
fn canonicalize_query_sorts_keys_and_drops_the_signature() {
    // 서명 자신은 canonical에서 빠지고, 나머지는 키 정렬 + 받은 인코딩 보존.
    let q = "X-Amz-Signature=zzz&X-Amz-Date=20260715T000000Z&X-Amz-Credential=AK%2F20260715%2Fauto%2Fs3%2Faws4_request";
    let c = canonicalize_query(q);
    assert!(!c.contains("X-Amz-Signature"));
    assert_eq!(
        c,
        "X-Amz-Credential=AK%2F20260715%2Fauto%2Fs3%2Faws4_request&X-Amz-Date=20260715T000000Z"
    );
}

#[test]
fn sign_matches_a_known_answer_vector() {
    // AWS SigV4(S3) 서명 체인의 자기정합 알려진 답 — secret·scope·
    // string-to-sign을 고정하면 서명은 이 hex다. 파이썬 hmac 참조와 대조.
    let string_to_sign = "AWS4-HMAC-SHA256\n\
         20130524T000000Z\n\
         20130524/us-east-1/s3/aws4_request\n\
         7344ae5b7ee6c3e7e6b0fe0640412a37625d1fbfff95c48bbb2dc43964946972";
    let got = sign(
        "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
        "20130524",
        "us-east-1",
        string_to_sign,
    );
    assert_eq!(
        got,
        "67fe34c8530db585abddc51067328adfedb6e42487d2566dc7d927d6e2722900"
    );
}
