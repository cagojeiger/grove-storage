# S3 호환성 점검

## 검증 경계

| 대상 | 이번 검증 |
|---|---|
| 서버 | Grove Storage 로컬 바이너리, 격리 PostgreSQL 17·filesystem |
| SDK | boto3/botocore 1.43.99, path-style, HTTP |
| 외부 S3 backend | 이번 E2E 대상과 구분, 실제 vendor 장애 검증은 후속 |
| 운영 | 배포·데이터 이관 없음 |
| 호환성 의미 | spec 03의 지원 subset, AWS 전체 API 동등성을 뜻하지 않음 |

## 결과

| 경계 | 근거 | 결과 |
|---|---|---|
| PUT·HEAD·GET·DELETE | `s3-capture.py` | 본문·크기·ETag·삭제 후 404 |
| Range | 같은 SDK 시나리오 | 206·부분 본문 |
| multipart | 자동 upload_file/download_file | 11 MiB 이상 병렬 업로드·합성 ETag·전체 다운로드 해시 |
| Abort | 같은 SDK 시나리오 | 다른 key 거부·정상 key 중단 |
| presigned | `e2e-s3.py`·`s3_auth_cases.py` | GET·Complete 정상 동작, 초과 만료 거부 |
| SigV4 경계 | protocol·API 단위 테스트 및 서명 요청 | 필수 host·정렬·중복·scope·만료 범위, 헤더 공백 정규화 |
| Complete 본문 변조 | `s3_auth_cases.py` | 서명 후 바뀐 바이트 400, 같은 세션의 정상 재시도 성공 |
| Complete XML | protocol 단위 테스트·실제 서명 요청 | 잘못된 문서 거부·namespace·엔티티·정상 재시도 |
| 미지원 요청 | protocol 분류·실제 SDK | CopyObject·ListParts·DeleteObjectTagging 501, 기존 객체 유지 |
| 완료·회수·GC | 기존 DB 통합 테스트 | 전체 workspace 회귀 실행 |
| Complete part 계약 | `completion` 단위 테스트·`s3_multipart_cases.py` | 5 MiB 경계·순서·중복·누락·ETag; 실패 시 기존 객체 보존·같은 UploadId 복구 |

## 재현 후 수정

| 이전 | 수정 |
|---|---|
| 문자열 split이 닫는 태그 없는 Complete 본문 수용 | roxmltree로 문서 파싱, 400 MalformedXML |
| CopyObject가 일반 PutObject로 처리되어 성공 | 객체 I/O 전에 501 NotImplemented |
| multipart·하위 리소스의 일반 객체 fallback | 지원 동작 명시 분류, 잘못된 조합 400·미지원 501 |
| 604801초 presigned URL·host 없는 서명 수용 | 인증 단계에서 403 AccessDenied |
| Complete 본문 바이트 변경 후 성공 | 상태 전이 전 400 XAmzContentSHA256Mismatch |
| 작은 비최종 part 완료·역순 목록 InvalidPart | EntityTooSmall·InvalidPartOrder로 구분, 완료 선점 전 거부 |

XML 실패는 이전 파서 단위 테스트에서, CopyObject 성공은 수정 전 실제 boto3
요청에서 재현했다. 운영 사고를 관찰했다는 의미와 구분한다.
만료 초과·host 누락·Complete 변조도 수정 전 실제 HTTP 200으로 재현했다.
작은 비최종 part의 완료 성공과 역순의 잘못된 오류 코드도 실제 SDK로 재현했다.

## 책임 구조

```text
api/s3                          HTTP·자격증명 조회·복호화·오류 응답
  -> s3-protocol/operation       요청 분류 (I/O 없음)
  -> s3-protocol/signing         서명 계산·raw query 정렬 (시계·DB 없음)
  -> s3-protocol/auth            scope·헤더·만료 범위·본문 해시 검증
  -> s3-protocol/multipart       Complete XML 파싱 (DB 없음)
  -> s3-protocol/completion      S3 완료 목록과 실측 원장 대조 (DB 없음)
  -> object-policy              공통 업로드 규칙·완료 관찰 판단
  -> object-service             정리·실패 보상 순서
  -> db                         파일 락·세션·원장·논리키 원자 전이
  -> infra                      filesystem·vendor S3 I/O
```

SigV4 요청 재료·시각 검증은 현재 API에 남아 있다. 트랜잭션을 분리하기보다
프로토콜 입력과 순수 판단부터 독립시킨다.

## 후속 검증

| 우선순위 | 경계 | 완료 기준 |
|---|---|---|
| 1 | SigV4 추가 호환성 | percent encoding 동등 표현·SDK별 서명 벡터·프록시/HTTPS 경로 |
| 2 | multipart 추가 옵션 | 추가 checksum 사용 시 연속 번호 등 별도 계약 검증 |
| 3 | 읽기·쓰기 옵션 | 조건부 요청·checksum·suffix Range의 지원/거부 계약 명시 |
| 4 | 실제 S3 backend | 동일 SDK 시나리오를 vendor 경유로 실행, timeout·응답 유실 복구 검증 |

현재 raw query 정렬 방식은 유지한다. 필수 query 인증 파라미터의 중복은 거부하며,
percent encoding의 모든 동등 표현까지 AWS와 같다고 판단하는 근거로 사용하지 않는다.
`UNSIGNED-PAYLOAD`의 본문 무결성은 서명 검증 범위와 구분한다.

## 실행

```sh
cargo test -p grove-s3-protocol --locked
cargo build --bin filegate --bin gscli --locked
python3 -m venv /tmp/grove-s3-sdk
/tmp/grove-s3-sdk/bin/pip install boto3==1.43.99
/tmp/grove-s3-sdk/bin/python scripts/e2e-s3.py
```

기준: [AWS CompleteMultipartUpload](https://docs.aws.amazon.com/AmazonS3/latest/API/API_CompleteMultipartUpload.html),
[roxmltree 파싱 옵션](https://docs.rs/roxmltree/0.21.1/roxmltree/struct.ParsingOptions.html).
인증 기준: [AWS query SigV4](https://docs.aws.amazon.com/AmazonS3/latest/developerguide/sigv4-query-string-auth.html),
[AWS header SigV4](https://docs.aws.amazon.com/AmazonS3/latest/developerguide/sig-v4-header-based-auth.html).
