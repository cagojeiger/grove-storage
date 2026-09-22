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
| presigned | `e2e-s3.py` | GET 실제 바이트 |
| Complete XML | protocol 단위 테스트·실제 서명 요청 | 잘못된 문서 거부·namespace·엔티티·정상 재시도 |
| 미지원 요청 | protocol 분류·실제 SDK | CopyObject·ListParts·DeleteObjectTagging 501, 기존 객체 유지 |
| 완료·회수·GC | 기존 DB 통합 테스트 | 전체 workspace 회귀 실행 |

## 재현 후 수정

| 이전 | 수정 |
|---|---|
| 문자열 split이 닫는 태그 없는 Complete 본문 수용 | roxmltree로 문서 파싱, 400 MalformedXML |
| CopyObject가 일반 PutObject로 처리되어 성공 | 객체 I/O 전에 501 NotImplemented |
| multipart·하위 리소스의 일반 객체 fallback | 지원 동작 명시 분류, 잘못된 조합 400·미지원 501 |

XML 실패는 이전 파서 단위 테스트에서, CopyObject 성공은 수정 전 실제 boto3
요청에서 재현했다. 운영 사고를 관찰했다는 의미와 구분한다.

## 책임 구조

```text
api/s3                          HTTP·자격증명 조회·복호화·오류 응답
  -> s3-protocol/operation       요청 분류 (I/O 없음)
  -> s3-protocol/signing         서명 계산·raw query 정렬 (시계·DB 없음)
  -> s3-protocol/multipart       Complete XML 파싱 (DB 없음)
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
| 1 | SigV4 엄격성 | host·SignedHeaders·scope·Expires 범위·Complete 본문 해시 변조 테스트 |
| 2 | multipart AWS 차이 | 비최종 part 최소 크기·InvalidPartOrder 등 실패 코드 계약 확정 |
| 3 | 읽기·쓰기 옵션 | 조건부 요청·checksum·suffix Range의 지원/거부 계약 명시 |
| 4 | 실제 S3 backend | 동일 SDK 시나리오를 vendor 경유로 실행, timeout·응답 유실 복구 검증 |

현재 raw query 정렬 방식은 유지한다. percent encoding의 동등 표현·중복 인증
파라미터까지 AWS와 동등하다고 판단하는 근거로 이 테스트를 사용하지 않는다.

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
