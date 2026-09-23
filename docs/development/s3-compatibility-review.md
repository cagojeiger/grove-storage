# S3 호환성 점검

## 검증 경계

| 대상 | 이번 검증 |
|---|---|
| 서버 | Grove Storage 로컬 바이너리, 격리 PostgreSQL 17·filesystem 또는 별도 MinIO 컨테이너 |
| SDK | boto3/botocore 1.43.99, path-style, HTTP |
| 외부 S3 backend | MinIO RELEASE.2025-09-07T16-13-09Z 경유 검증; AWS S3·R2·운영 endpoint는 별도 |
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
| 요청 무결성 | `s3_integrity_cases.py`·spool 테스트 | PUT checksum·UploadPart signed hash 실패 시 기존 객체/part 유지, 정상 MD5·CRC32·SHA256 |
| 조건부 요청 | 같은 SDK 시나리오 | GET/HEAD If-Match 정상·stale 412, 미지원 조건부 쓰기 501 |
| vendor 데이터 | `s3_backend_fixture.py` | 실제 MinIO bucket의 바이트 일치, 시나리오 종료 시 열린 multipart 0개 |
| vendor 가용성 | 같은 fixture의 stop/start | 중지 중 GET 503 ServiceUnavailable, 같은 주소 재시작 후 동일 객체 읽기 |
| Complete 응답 유실 | `e2e-s3-recovery.py`·`s3_fault_proxy.py` | MinIO 성공 후 응답 차단, 기존 객체 보존, 재시도 fencing, 실물 관찰 확정·purge·점유 정산 |
| 응답 유실 후 프로세스 재시작 | `e2e-s3-recovery.py --restart` | SIGKILL 종료·새 PID 확인, 동일 DB·endpoint, 소유권 보존과 새 Reconciler 복구 |
| DB 커밋 거부 | `e2e-s3-recovery.py --db-failure`·`s3_db_fault.py` | vendor 성공, 요청·Reconciler 커밋 롤백, 장애 제거 후 실물 관찰 확정·정산 |

## 재현 후 수정

| 이전 | 수정 |
|---|---|
| 문자열 split이 닫는 태그 없는 Complete 본문 수용 | roxmltree로 문서 파싱, 400 MalformedXML |
| CopyObject가 일반 PutObject로 처리되어 성공 | 객체 I/O 전에 501 NotImplemented |
| multipart·하위 리소스의 일반 객체 fallback | 지원 동작 명시 분류, 잘못된 조합 400·미지원 501 |
| 604801초 presigned URL·host 없는 서명 수용 | 인증 단계에서 403 AccessDenied |
| Complete 본문 바이트 변경 후 성공 | 상태 전이 전 400 XAmzContentSHA256Mismatch |
| 작은 비최종 part 완료·역순 목록 InvalidPart | EntityTooSmall·InvalidPartOrder로 구분, 완료 선점 전 거부 |
| If-None-Match 쓰기·잘못된 MD5/CRC32·변조된 UploadPart 성공 | 미지원 조건 501, checksum·서명 hash 대조 후에만 승격 |

XML 실패는 이전 파서 단위 테스트에서, CopyObject 성공은 수정 전 실제 boto3
요청에서 재현했다. 운영 사고를 관찰했다는 의미와 구분한다.
만료 초과·host 누락·Complete 변조도 수정 전 실제 HTTP 200으로 재현했다.
작은 비최종 part의 완료 성공과 역순의 잘못된 오류 코드도 실제 SDK로 재현했다.
조건부 쓰기·잘못된 checksum·UploadPart 변조도 수정 전 SDK/서명 요청에서 재현했다.
boto3 병렬 다운로드가 If-Match를 사용하는 것을 회귀 테스트로 확인해 읽기 조건을 구현했다.

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
| 3 | 읽기·쓰기 추가 옵션 | 원자적 조건부 쓰기·checksum 저장/조회/전체 multipart·suffix Range |
| 4 | 쓰기 장애 복구 | DB COMMIT 응답 유실·쓰기 진행 도중 종료, AWS S3/R2 별도 호환성 |

현재 raw query 정렬 방식은 유지한다. 필수 query 인증 파라미터의 중복은 거부하며,
percent encoding의 모든 동등 표현까지 AWS와 같다고 판단하는 근거로 사용하지 않는다.
`UNSIGNED-PAYLOAD`의 본문 무결성은 서명 검증 범위와 구분한다.

## 실행

```sh
cargo test -p grove-s3-protocol --locked
cargo build --bin filegate --bin gscli --locked
python3 -m venv /tmp/grove-s3-sdk
/tmp/grove-s3-sdk/bin/pip install boto3==1.43.99
/tmp/grove-s3-sdk/bin/python -B -u scripts/e2e-s3.py --backend fs
/tmp/grove-s3-sdk/bin/python -B -u scripts/e2e-s3.py --backend minio
/tmp/grove-s3-sdk/bin/python -B -u scripts/e2e-s3-recovery.py
/tmp/grove-s3-sdk/bin/python -B -u scripts/e2e-s3-recovery.py --restart
/tmp/grove-s3-sdk/bin/python -B -u scripts/e2e-s3-recovery.py --db-failure
```

두 모드는 같은 SDK 성공·거부·재시도 시나리오를 사용한다. MinIO 모드는 테스트가
직접 만든 컨테이너만 중지·재시작하며 기존 운영 endpoint를 받지 않는다. 컨테이너·
볼륨·PG·임시 파일은 finally에서 정리한다. 재시작 시 endpoint 유지가 계약이므로
Docker 자동 포트 재할당 대신 명시적 임시 포트를 사용한다.

이번 단계는 테스트/CI/문서 변경이다. 제품 Rust 코드는 변경하지 않았다.
### 완료 응답 유실 검증

| 시점 | 확인 |
|---|---|
| MinIO Complete 성공 | 프록시가 실제 200 응답을 읽은 뒤 연결 종료; vendor 바이트 직접 대조 |
| Grove 응답 실패 | 503, DB completing 유지, 논리키는 이전 객체 반환 |
| 같은 UploadId 재시도 | 503, 추가 vendor Complete 없음 |
| lease 만료 후 | 실제 Reconciler가 HEAD로 관찰해 active·committed로 확정 |
| 복구 이후 | 새 객체 1개, 이전 실물 삭제, 예약·purge 대기 0, 실제 크기로 점유 정산 |
| 확정된 UploadId 재요청 | NoSuchUpload, 추가 vendor Complete 없음 |

격리 DB의 대상 write lease만 과거 시각으로 바꾸고 Reconciler 주기를 1초로 설정한다.
실제 15분 대기·쓰기 진행 도중 종료·DB COMMIT 응답 유실을 검증한 결과와 구분한다.
프록시는 loopback MinIO만 대상으로 하며 Host·경로·HEAD Content-Length를 보존한다.

### 응답 유실 후 재시작

`--restart`는 Grove가 503을 반환하고 DB가 completing으로 남은 시점에 SIGKILL한다.
종료 코드와 새 PID를 확인하고, 동일 DB·암호화 키·endpoint로 서버를 재실행한다.

| 시점 | 확인 |
|---|---|
| 재시작 직후, lease 유효 | pending/completing 유지, 이전 객체 읽기, Complete 재시도 503, vendor 중복 호출 0 |
| 대상 lease 만료 후 | 새 Reconciler가 새 객체 확정, write lease committed, 이전 실물 purge |
| 정산 | 활성 객체 1개·실제 크기, 예약·purge 대기 0, 열린 multipart 0 |

실행 중인 Complete 요청을 강제 종료하는 경우와 구분한다. 기본 모드와 재시작 모드를
CI에서 각각 실행하며, CLI fixture의 기존 실행 경로도 로컬 E2E로 확인했다.

### DB 커밋 거부

`--db-failure`는 Complete 응답을 정상 전달한다. 격리 PostgreSQL의 대상 file_id에만
deferred constraint trigger를 적용해 세션 삭제 후 COMMIT에서 예외를 발생시킨다.
rollback에 영향받지 않는 sequence로 요청과 Reconciler 양쪽의 주입 실행을 확인한다.

| 시점 | 확인 |
|---|---|
| 요청 COMMIT 거부 | 500 InternalError; MinIO Complete 200·물리 바이트 일치 |
| 클라이언트 재시도 | 503, 추가 vendor Complete 없음 |
| lease 만료 후 Reconciler COMMIT 거부 | pending/completing·issued 유지, 이전 논리키 보존, 이전 active·새 reserved 점유 유지 |
| 트리거 제거 후 | 자동 확정, 이전 실물 purge, 활성 객체 1개·예약 및 purge 대기 0 |

실패 시에도 fixture DB 전체를 폐기한다. 이 검증은 확실한 rollback이며,
DB가 COMMIT한 뒤 응답만 유실되는 결과 불명확성이나 DB 전체 장애와 구분한다.

기준: [AWS CompleteMultipartUpload](https://docs.aws.amazon.com/AmazonS3/latest/API/API_CompleteMultipartUpload.html),
[roxmltree 파싱 옵션](https://docs.rs/roxmltree/0.21.1/roxmltree/struct.ParsingOptions.html).
인증 기준: [AWS query SigV4](https://docs.aws.amazon.com/AmazonS3/latest/developerguide/sigv4-query-string-auth.html),
[AWS header SigV4](https://docs.aws.amazon.com/AmazonS3/latest/developerguide/sig-v4-header-based-auth.html).
