# spec 09: 공통 자원 명령 계약

- 상태: `grove-management-command` + 조회 13개·변경 11개 실행기 + Bearer HTTP 구현.
- CLI의 원격 24개 명령과 MCP 24개 tool을 같은 실행기에 연결했다. [MCP 전송 계약](10-management-mcp.md).
- 현행 인증: [spec 11](11-local-management-auth.md). 권한·감사 경계: [spec 08](08-management-plane.md). CLI: [spec 04](04-cli.md).

## 구성

```text
management-policy       Caller + Surface + Action → Scope / 거부
        ↑
management-command      명령명 + 입력/출력 + schema + 권한 + 변경 여부
        ↑
gscli                   인자·파일·확인 → commands HTTP → 표 / JSON

commands HTTP → resources 실행기 → 신원 잠금·현재 권한 → DB 조회 / 변경 + 감사
Storage 생성/교체: 권한 확인 → 잠금 해제 → 접근 probe → 현재 권한 재확인 → 변경 + 감사
MCP tools/call → 같은 resources 실행기 (surface=mcp)
```

| 항목 | 현재 계약 |
|---|---|
| protocol | `COMMAND_PROTOCOL_VERSION = 1`; 패키지 버전·CLI JSON envelope 버전과 별개 |
| 명령 정본 | `CommandName::ALL`, `as_str`, `required_action`, `effect` |
| schema | 입력·출력 타입에서 JSON Schema 생성; CLI/MCP adapter가 같은 정의를 사용 |
| 입력 검사 | `decode(protocol, name, input)`: protocol → 명령명 → 타입 → 값 검사 |
| 출력 검사 | `decode_output`: 응답 타입 검사; 요청과 결과의 ID 대조는 adapter/service 책임 |
| 인증 | 명령 metadata의 Action을 검증된 Caller·Surface와 함께 정책에 전달 |
| 로컬 처리 | `--from`, `--key-file`, `--secret-out`, `--yes`, update는 CLI 소유 |

CLI는 typed 입력 검증·직렬화와 응답 검증을 수행하고, 현재 권한은 서버가 적용한다.
공통 HTTP 경로·envelope는 아래 계약을 사용하며 MCP tool명은 같은 명령명을 사용한다.

## 공통 자원 HTTP (4a·4b)

```http
POST /api/admin/commands/v1
Authorization: Bearer gsm_...
Content-Type: application/json

{"protocol":1,"command":"storage.list","input":{}}
```

| 항목 | 현재 계약 |
|---|---|
| 지원 범위 | 조회 13개·변경 11개, catalog의 24개 전체 |
| 성공 | 200 `{protocol:1, request_id, command, result}`; result는 공통 typed Output |
| 실패 | `{protocol:1, request_id, error:{code,outcome}}`; commit 전 거부는 `not_applied`, 변경 commit 실패는 `unavailable/unknown` |
| HTTP 코드 | unauthorized 401, forbidden 403, not_found 404, conflict 409, unavailable 503; 입력/protocol/명령/접근 검사 거부는 400 |
| 인증 | 활성 User 관리 토큰; Cookie가 있거나 Authorization이 중복이면 401; master·기존 운영자 토큰은 별도 namespace |
| 표면 | 서버가 `resource_api`로 기록; 요청 body·User-Agent·proxy 사용자 헤더로 actor/surface 지정 생략 |
| 응답 보안 | no-store·nosniff·서버 request_id; 관리 쿠키 발급·OAuth redirect·CORS 허용 생략 |
| 조회 경계 | identity lock → 현재 신원·role·owner 확인 → 조회 → commit; `storage.test`만 별도 외부 probe 후 신원·설정 재확인 |
| DB 재사용 | 기존 registry/usage/S3 키 SQL이 pool 또는 현재 transaction에서 실행; SQL 조건·정렬 유지 |
| 변경 경계 | 현재 권한 → 자원 변경 + audit → commit; 감사 실패는 rollback, commit 불명은 자동 재시도 없이 대조 |
| 상태 | CLI도 단일 status 명령 사용; 서버의 신원/DB/등록부 관찰, 물리 storage는 `not_checked`; 실패 시 명령 오류 |
| 일관성 | READ COMMITTED; 여러 SELECT의 등록부 상태를 고정 snapshot으로 보장하지 않음 |
| 호출 기록 | service 실행 시 검증된 actor + 명령명 + 결과를 한 번 기록; 조회는 변경 감사 생성 없이 완료 |
| 거부 기록 | 인증 실패는 보안 이벤트, 신원 확인 후 권한 거부는 호출·보안 이벤트; envelope/decode 거부는 HTTP 진단 범위 |
| 비밀 | 응답은 명시적 DTO; S3 secret은 발급 commit 성공 시 한 번 전달; 로그는 요청/결과 payload 대신 ID·안정 코드 사용 |

CLI HTTP는 User 토큰을 사용하며 서버가 `resource_api`로 기록한다.
CLI/MCP Surface 동등성은 같은 실행기의 PG 정책 테스트다. 실제 CLI 전송은 E2E로
검증하며 실제 MCP HTTP도 CLI 결과와 대조한다. 기존 `/api/admin/v1`은
인증·DB 접근 없이 410을 반환한다.

### 브라우저 전송

`POST /api/admin/console-commands/v1`은 같은 24개 명령과 응답 계약을 사용한다.
비밀번호 로그인 세션 쿠키·정확한 Origin·`X-Grove-CSRF: 1`을 검사한 뒤 같은 실행기에 연결하며
표면은 서버가 `console`로 지정한다. Bearer·기존 운영자 쿠키는 거부한다.
진입 guard의 401/403은 신원 API 오류 형식이고 실행기 진입 후 오류는 공통 envelope다.
현재 UI는 개요·저장소·Clients·서비스 키 명령을 연결한다. 신원/이력은 별도 세션 API에 유지한다.

## 명령 목록

| 명령 | 입력 | 출력 | 권한 | 효과 |
|---|---|---|---|---|
| `status` | `{}` | Status | ReadResources | 조회 |
| `storage.list` | `{}` | Storage[] | ReadResources | 조회 |
| `storage.show` | id | Storage | ReadResources | 조회 |
| `storage.metadata.show` | id | ResourceMetadata | ReadResources | 조회 |
| `storage.metadata.replace` | id, metadata | ResourceMetadata | WriteResources | 변경 |
| `storage.test` | id | StorageConnection `{id,state:"ok"}` | ReadResources | 조회·외부 probe |
| `storage.create`, `storage.replace` | id, spec | Storage | WriteResources | 변경 |
| `storage.delete` | id | Deleted | WriteResources | 변경 |
| `client.list` | `{}` | string[] | ReadResources | 조회 |
| `client.show` | id | Client | ReadResources | 조회 |
| `client.metadata.show` | id | ResourceMetadata | ReadResources | 조회 |
| `client.metadata.replace` | id, metadata | ResourceMetadata | WriteResources | 변경 |
| `client.create` | id, storage_id | Client | WriteResources | 변경 |
| `client.delete` | id | Deleted | WriteResources | 변경 |
| `credential.list` | client_id | string[] | ManageServiceCredentials | 조회 |
| `credential.create` | client_id | IssuedCredential | ManageServiceCredentials | 변경 |
| `credential.delete` | client_id, access_key_id | Deleted | ManageServiceCredentials | 변경 |
| `client-key.list` | client_id | string[] | ManageServiceCredentials | 조회 |
| `client-key.register` | client_id, key_hash | ClientKey | ManageServiceCredentials | 변경 |
| `client-key.delete` | client_id, key_hash | Deleted | ManageServiceCredentials | 변경 |
| `usage.storages` | `{}` | StorageUsage[] | ReadResources | 조회 |
| `usage.clients` | `{}` | ClientUsage[] | ReadResources | 조회 |
| `usage.history` | days (기본 90) | Snapshot[] | ReadResources | 조회 |

24개 명령은 조회 13개·변경 11개다. Account·role·관리 토큰·관리 이력은
콘솔 세션 API의 별도 계약이다. `credential`은 Client의 S3 서비스 키를 뜻한다.

Metadata는 문자열 값만 가진 JSON object이며 구분자 공백을 포함한 정규화 JSON은 최대 8 KiB다.
키·값의 NUL은 거부한다. replace는 전체 교체이고 `{}`는 비우기다.
Metadata 원문은 관리 감사에 저장하지 않는다.

### Client·서비스 키 변경 (4b-1)

| 경계 | 현재 계약 |
|---|---|
| 생성 | 기존 slug·예약 Client명·storage/client FK·키 중복 제약 적용 |
| 삭제 | 파일 참조가 남은 Client는 409; 키는 client_id와 대상 키를 함께 대조 |
| 멱등 | 실제 삭제 행이 있을 때만 감사 생성; Client 삭제의 소유 키 cascade는 `client.delete` 한 건으로 기록 |
| Native 키 감사 | `resource_type=client_key`, target·metadata는 Client ID; 원문·전체 hash 제외, 개별 키 식별은 현 감사 범위 밖 |
| S3 키 감사 | `resource_type=s3_credential`, target은 공개 access key ID, metadata는 Client ID |
| Client 감사 | target은 Client ID; 생성 metadata는 Client/Storage ID, 삭제는 Client ID |
| S3 암호화 | 주입된 기존 Crypto·활성 key ID·access key ID AAD 사용; 기존 SigV4 인증 저장 형식 유지 |
| 불명확한 발급 | `unknown`이면 원문 반환 없이 종료; 목록 대조·필요 시 폐기 후 명시적 재발급 |

이 보장은 새 공통 실행기 범위다. 기존 `/api/admin/v1`은 기존 인증·기록을 유지한다.

### Storage 변경 (4b-2)

```text
identity lock → 인증·권한·입력·교체 대상 확인 → 읽기 transaction 종료
    → S3 접근 검사·Provider Secret 암호화 (DB 잠금 밖)
    → identity lock → 현재 토큰·role·User 상태 재확인
    → Storage 행 잠금·현재 참조 검사 → 변경 + 감사 commit
```

| 경계 | 계약 |
|---|---|
| 검사 재사용 | 기존 REST의 필드·URL·relay 설정 검사; S3 HeadBucket + ListMultipartUploads; FS create/replace rejected before probe |
| 권한 변경 | 검사 중 폐기·강등·owner 변경 상태를 commit 전 재확인; 거부 시 등록부 변경 없이 종료 |
| 교체 | 없는 대상은 probe 전 404; 검사 중 삭제된 대상도 404, 검사 중 생성된 참조도 최종 판단에 포함 |
| 주소 | 기존 Storage 행 잠금과 파일 예약 직렬화 유지; locations가 남은 물리 주소 변경은 409 |
| 운영 변경 | 파일이 있어도 용량·Provider 키 교체 허용; 암호화 AAD·key ID 형식 유지 |
| 삭제 | Client 또는 location 참조가 남으면 409; 실제 삭제 때만 감사, 반복 삭제는 성공 |
| 감사 | Storage ID·종류/용량/relay 전후 값·주소 변경 여부; 주소·경로·access key·secret·암호문 제외 |
| 오류 | probe 상세는 공통 응답과 관리 이력에서 제외; 운영 로그도 Provider 오류 원문 대신 storage ID·kind 기록 |
| 원자성 | DB 변경·감사만 같은 transaction; 접근 검사는 시점 관찰이며 외부 I/O의 rollback·지속 가용성과 구분 |

서비스는 접근 검사 함수를 주입받는다. 서비스 테스트는 제어 가능한 검사 대역으로 경합을
검증하고, API 테스트는 로컬 HTTP S3 대역과 FS 등록 거부를 확인한다.
현행 backend는 S3-only이며 migration `0017`은 FS 행이 남으면 중단한다.

## 검증과 비밀

| 경계 | 검사·보장 |
|---|---|
| 입력 구조 | object만 허용; 알 수 없는 필드 거부; storage spec도 동일 |
| Backend | S3-only; 조회 DTO의 호환 필드 `root_path`는 null이며 FS 실행을 의미하지 않음 |
| 값 | ID의 빈 값·dot segment·제어 문자 거부, history 1–3650, 용량 0 이상 |
| 서비스 키 | Native key는 `sha256:` + 소문자 hex 64자; S3 access ID는 소문자 영숫자 8–64자 |
| 서비스 검증 | 생성 slug·예약 Client명·S3 필수 필드·URL·접근 probe·참조/삭제 조건 |
| 응답 | 기존 CLI DTO 형태 유지; 알 수 없는 응답 필드는 typed 출력에서 제외 |
| 비밀 전달 | S3 발급 응답의 secret_key는 의도한 일회성 전달; CLI는 기존 비밀 파일 저장 유지 |
| 진단 | Command/Output의 Debug는 명령명만 출력; StorageSpec/IssuedCredential은 Debug 미제공 |
| 감사 | Serialize는 전달용; 저장 로그는 별도 allowlist와 크기 제한 적용 |

JSON Schema는 입력 형태를 설명하며 값·서비스 검사를 대체하지 않는다.
CLI `status`는 서버 관찰 결과다. 이전 CLI의 5개 HTTP 진단 호출을 단일 명령으로
교체했다. 호출 실패 시 부분 상태를 생성하지 않으며 물리 저장소는 `not_checked`다.

## 오류와 결과

| 안정 코드 | 의미 |
|---|---|
| protocol_incompatible / unknown_command / invalid_input | 실행 전 계약 거부 |
| unauthorized / forbidden | 인증 / 권한 거부 |
| not_found / conflict / request_rejected | 서비스의 대상 / 상태 / 요청 거부 |
| invalid_response / unavailable / internal | 응답 형식 / 가용성 / 내부 실패 |

`CommandError`는 `code`와 `outcome`만 제공한다. Provider·serde 오류 원문은 포함하지 않는다.

| outcome | 의미 |
|---|---|
| not_applied | 실행 전 거부 등 변경 미적용의 증거가 있음 |
| applied | 변경 확정 증거가 있음 |
| unknown | 전송 단절·응답 불명 등 적용 여부를 확정할 수 없음 |

오류 코드만으로 변경 결과를 추론하지 않는다. `rejected` 생성자는 실행 전 거부에 사용한다.
CLI 출력 envelope는 `schema_version: 1`을 유지한다. 안정 코드·outcome·종료 코드의
대응은 [spec 04](04-cli.md)를 따른다. 변경 응답을 검증할 수 없으면 `unknown`이다.

## 검증

| 위치 | 검증 대상 |
|---|---|
| `management-command/tests/catalog.rs` | 24개 명령·권한·효과·CLI/MCP 정책 동등성·schema 구조 |
| `management-command/tests/inputs.rs` | 필수/미지원 필드·경계값·기본값·hash 입력 |
| `management-command/tests/outputs.rs` | wire fixture 왕복·비밀 필드 제거·진단 redaction |
| `management-command/tests/errors.rs` | protocol 거부 순서·안정 코드·적용 결과 분리 |
| `cli/tests/command_contract.rs` | 실제 clap 원격 명령과 catalog의 일대일 대응 |
| `cli/tests/` | command HTTP·protocol/명령/대상 ID 검증·JSON·설정 우선순위·비밀 파일·변경 결과·단일 status |
| `scripts/e2e-cli.py` | 실제 CLI/서버/PG의 등록부 수명주기·기존 REST 결과 비교·Account 강등·폐기·감사·비밀 제외 |
| `api/src/mcp/tests/`, `scripts/e2e-mcp.py` | 24개 tool schema·실제 HTTP/CLI 결과·MCP 감사·owner·폐기·rollback·unknown·서버 비밀 로그 제외 |
| `management-service/tests/resources.rs` + `resources/` | CLI/MCP/API 권한 동등성·User 현재 역할·잠금 대기 후 role 재확인·호출 한 번·DB/로그 실패 |
| `management-service/tests/resource_writes.rs` + `resource_writes/` | 8개 PG 테스트: 변경 6개·암호화·현재 권한·소유 범위·삭제 제약·감사 rollback·commit unknown·telemetry 장애 |
| `api/src/commands/tests/` | 조회·metadata·기존 REST 응답 비교·Cookie/Bearer 분리·입력/표면 검증·폐기·비밀 제외 |
| 같은 경로의 `writes.rs`, `write_failures.rs` | 4개 PG HTTP 테스트: 변경 왕복·기존 키 조회/인증·409/400·감사 rollback·원문 없는 unknown |
| `management-service/tests/storage_writes.rs` + `storage_writes/` | 8개 PG 테스트: 변경 3개·참조·probe 중 폐기/User 강등·참조 경합·감사 rollback·unknown |
| `api/src/commands/tests/storage*.rs` | PG HTTP 테스트: FS 등록/교체 거부·기존 REST 결과·필드/설정 검사·S3 대역 probe/키 교체·비밀 제외·감사/commit 장애 |
| 로컬 서버 smoke | 임시 PG·실제 프로세스에서 조회 11개·User 현재 역할·폐기·기존 REST 유지 확인; HTTP 헤더 직접 전송, TLS/브라우저/proxy와 구분 |
| 변경 서버 smoke | 변경 6개·기존 S3 키 목록·Native PUT/commit/GET 바이트 일치·파일 참조 삭제 409·키 폐기 후 401·비밀 없는 감사 확인; S3 실제 전송은 이번 검증에서 제외 |
| Storage 서버 smoke (이전 단계 기록) | 임시 PG·실제 프로세스의 생성/교체/삭제·기존 REST 조회 일치·당시 fs 바이트 왕복·파일 존재 중 용량 변경·주소/삭제 409·멱등 삭제·주소 없는 감사 확인 |

위 개수·smoke 결과는 각 단계의 기록이다. 현재 명령 정본은 `management-command/src/catalog.rs`,
S3-only/MinIO·NoteGate 검증은 [호환성 점검](../development/s3-compatibility-review.md)을 따른다.
현재 검증은 순수 계약·PG 서비스·HTTP 라우터와 자원 변경/audit transaction을 포함한다.
외부 MCP 앱·운영 proxy는 별도 검증이다. S3 대역 검증은 외부 Provider 운영 검증과 구분한다.
