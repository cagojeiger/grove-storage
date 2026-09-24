# spec 09: 공통 자원 명령 계약

- 상태: `grove-management-command` 구현. CLI가 명령명·입력/응답 DTO를 재사용한다.
- 서버 공통 실행기·새 HTTP 진입점·MCP adapter는 후속 단계다. 기존 CLI REST·인증·출력은 유지한다.
- 권한·신원·감사: [spec 08](08-management-plane.md). 현행 CLI: [spec 04](04-cli.md).

## 구성

```text
management-policy       Caller + Surface + Action → Scope / 거부
        ↑
management-command      명령명 + 입력/출력 + schema + 권한 + 변경 여부
        ↑
gscli                   인자·파일·확인 → 기존 REST → 표 / JSON

후속: 서버 실행기 ← CLI HTTP adapter / MCP tool adapter
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

현재 CLI는 새 `decode`·권한 정책·오류 타입을 실행하지 않는다. 공통 실행기 연결 시
검증과 권한 적용을 함께 전환한다. MCP tool명·HTTP path·전송 envelope는 그 단계에서 확정한다.

## 명령 목록

| 명령 | 입력 | 출력 | 권한 | 효과 |
|---|---|---|---|---|
| `status` | `{}` | Status | ReadResources | 조회 |
| `storage.list` | `{}` | Storage[] | ReadResources | 조회 |
| `storage.show` | id | Storage | ReadResources | 조회 |
| `storage.create`, `storage.replace` | id, spec | Storage | WriteResources | 변경 |
| `storage.delete` | id | Deleted | WriteResources | 변경 |
| `client.list` | `{}` | string[] | ReadResources | 조회 |
| `client.show` | id | Client | ReadResources | 조회 |
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

19개 명령은 조회 10개·변경 9개다. User·Agent·role·관리 토큰·관리 이력은
콘솔 세션 API의 별도 계약이다. `credential`은 Client의 S3 서비스 키를 뜻한다.

## 검증과 비밀

| 경계 | 검사·보장 |
|---|---|
| 입력 구조 | object만 허용; 알 수 없는 필드 거부; storage spec도 동일 |
| 값 | ID의 빈 값·dot segment·제어 문자 거부, history 1–3650, 용량 0 이상 |
| 서비스 키 | Native key는 `sha256:` + 소문자 hex 64자; S3 access ID는 소문자 영숫자 8–64자 |
| 서비스 검증 | 생성 slug·예약 Client명·S3/fs 필수 필드·URL·접근 probe·참조/삭제 조건 |
| 응답 | 기존 CLI DTO 형태 유지; 알 수 없는 응답 필드는 typed 출력에서 제외 |
| 비밀 전달 | S3 발급 응답의 secret_key는 의도한 일회성 전달; CLI는 기존 비밀 파일 저장 유지 |
| 진단 | Command/Output의 Debug는 명령명만 출력; StorageSpec/IssuedCredential은 Debug 미제공 |
| 감사 | Serialize는 전달용; 저장 로그는 별도 allowlist와 크기 제한 적용 |

JSON Schema는 입력 형태를 설명하며 값·서비스 검사를 대체하지 않는다.
`status`는 현재 CLI의 HTTP 관찰 결과다. 물리 저장소 접근은 `not_checked`로 유지하며
서버 관찰과 CLI 네트워크 관찰의 차이는 adapter 연결 단계에서 검증한다.

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
새 계약은 기존 CLI의 오류 envelope·종료 코드를 변경하지 않는다.

## 검증

| 위치 | 검증 대상 |
|---|---|
| `management-command/tests/catalog.rs` | 19개 명령·권한·효과·CLI/MCP 정책 동등성·schema 구조 |
| `management-command/tests/inputs.rs` | 필수/미지원 필드·경계값·기본값·hash 입력 |
| `management-command/tests/outputs.rs` | wire fixture 왕복·비밀 필드 제거·진단 redaction |
| `management-command/tests/errors.rs` | protocol 거부 순서·안정 코드·적용 결과 분리 |
| `cli/tests/command_contract.rs` | 실제 clap 원격 명령과 catalog의 일대일 대응 |
| 기존 CLI 테스트 | REST path·JSON·비밀 파일·변경 결과·status 동작 유지 |

순수 계약 검증과 실제 MCP 전송·DB transaction·새 인증 검증은 별도 단계다.
