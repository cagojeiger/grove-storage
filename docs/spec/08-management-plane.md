# spec 08: 관리 신원·명령·감사

- 상태: 순수 권한·명령 계약, 신원 DB·원자적 변경 감사 구현·테스트. 새 인증 API·MCP·UI 미연결.
- 결정: [ADR 009](../adr/009-management-identity-and-command-boundary.md).
- 현재 구현: [인증](05-admin-auth.md), [CLI](04-cli.md), [콘솔](06-console.md).
- `0008`은 `management` 스키마를 추가한다. 현재 API는 기존 `admin_*`를 계속 사용한다.

## 책임과 접근

| 영역 | 주체·기능 | 경로 |
|---|---|---|
| 신원·권한 | User·Agent·관리 토큰·세션 관리 | 대시보드 전용 API |
| 자원 운영 | Storage·Client·서비스 키·usage/status | 대시보드, CLI, MCP |
| 관리 이력 | 변경 감사·관리 호출·보안 이벤트 조회 | 대시보드 전용 API |
| 데이터 서비스 | Native/S3 파일 접근·Client 키 인증 | 기존 runtime API |
| 설치 복구 | 최초 Admin 생성·관리 접근 복구 | 마스터 인증의 전용 콘솔 흐름 |

서버는 `주체 + 자격증명 종류 + 진입 경계 + 작업 권한`을 함께 검사한다.
User-Agent 헤더·Origin 문자열·도구 이름은 콘솔 권한의 증거가 아니다.
직접 HTTP 호출도 같은 검사를 거치며 Agent 토큰은 사람의 콘솔 세션을 발급받지 않는다.
콘솔 전용은 세션·역할·CSRF의 인증 경계이며 실제 사람이 화면을 조작했다는 증명은 아니다.

## 권한

| 작업 | Viewer | Operator | Admin | 허용 진입점 |
|---|---|---|---|---|
| Storage·Client·usage/status 조회 | 허용 | 허용 | 허용 | 콘솔·CLI·MCP |
| Storage·Client 등록·변경·삭제 | 거부 | 허용 | 허용 | 콘솔·CLI·MCP |
| Native/S3 서비스 키 목록·발급/등록·폐기 | 거부 | 허용 | 허용 | 콘솔·CLI·MCP |
| 자기 세션 조회·종료 | 허용 | 허용 | 허용 | 콘솔 세션 |
| User·Agent·역할·관리 토큰 조회/생성/변경/폐기 | 거부 | 거부 | 허용 | 콘솔 세션 |
| 관리 변경·호출 이력 조회 | 자기 범위 | 자기 범위 | 전체 | 콘솔 세션 |
| 전체 보안 이벤트 조회 | 거부 | 거부 | 허용 | 콘솔 세션 |

자기 범위는 본인과 소유 Agent의 관리 이력이며 조회 시 현재 권한을 확인한다.
Operator는 Client 서비스 키를 다루는 강한 운영 권한이다. 발급 화면에서 이를 명시한다.
기존 삭제·참조·주소 변경 제약은 Admin과 master를 포함한 모든 경로에서 유지한다.
Agent는 viewer/operator로 제한하고 소유 User의 활성 상태·권한으로 상한을 적용한다.
역할 변경·비활성화는 이후 요청에 반영하며 이미 허용된 작업은 완료될 수 있다.
마지막 활성 Admin의 삭제·비활성화·강등과 첫 Admin 생성은 DB 잠금으로 직렬화한다.

### 정책 구현 경계

`grove-management-policy`는 I/O·외부 dependency 없이 검증된 Caller snapshot과
서버가 정한 Surface, Action을 받아 `Scope` 또는 거부 사유를 반환한다.

| 현재 구현 | 연결 단계의 책임 |
|---|---|
| User/Agent 역할·활성 상태·소유자 권한 상한 | DB에서 현재 상태를 읽고 알 수 없는 role/kind를 거부 |
| credential 만료/폐기/invalid 상태 거부 | token 검증·세션과 원본 token 결합·master 세대 검증 |
| 콘솔 세션과 machine Bearer의 조합 검사 | 서버 route에서 Surface 지정, Origin/CSRF와 쿠키 검증 |
| Installation / SelfOnly / SelfAndOwnedAgents / SetupRecovery | 인증된 ID로 조회/변경 범위 적용; 기존 자원 제약 유지 |

정책 허용은 DB 쿼리의 소유권 필터·첫/마지막 Admin 잠금·삭제 조건을 대신하지 않는다.
현재 API의 기존 인증 경로는 아직 이 crate를 호출하지 않는다. CLI/MCP 동등성 테스트는
정책 입력에 대한 결과 비교다. [명령 계약](09-management-commands.md)은 실제 CLI 명령
목록·입출력 DTO·schema를 검증하며 두 adapter의 전송 검증은 후속이다.

## 초기 설정·로그인·복구

```text
설정 master → 제한된 설정 세션 → 최초 Admin + 개인 토큰 발급
개인 토큰   → User 확인        → 일반 콘솔 세션
User/Agent 토큰                → CLI/MCP 자원 명령
설정 master → 제한된 복구 세션 → 기존 Admin 접근 복구
```

| 대상 | 계약 |
|---|---|
| master | Secret 파일·환경변수로 공급; DB에 원문 저장 없이 검증 |
| 설정 세션 | 짧은 TTL, 최초 설정·복구에만 사용; 일반 자원·감사 조회 API와 분리 |
| 첫 User | Admin으로 생성; User·개인 토큰·감사 이벤트를 함께 commit |
| 이후 User | Admin이 콘솔에서 생성·역할 부여·개인 토큰 전달 |
| 일반 세션 | User·원본 credential에 연결; 원본 토큰 만료 이내의 고정 TTL |
| 폐기 | 토큰 폐기 시 연결 세션 거부; 계정 비활성화 시 관련 접근 거부 |
| master 교체 | 설정 세대 변경으로 기존 master 세션 무효화; replica 불일치 시 master 경로 fail-closed |
| 복구 | 대상 Admin의 개인 토큰 재발급·기존 토큰/세션 폐기; 전체 관리 credential 폐기는 별도 명시적 선택 |
| 복구 영향 | Client 키·Provider secret·파일 데이터는 보존; 수행 내역은 master 주체로 감사 |
| 원문 유실 | 발급 원문 재조회 대신 공개 credential ID로 대조 후 폐기·재발급 |

비밀번호·메일 복구·공개 가입은 제공 범위 밖이다. master는 명명된 User를 가장하지 않는다.
향후 OIDC 연결은 `(issuer, subject) → user_id`로 명시적으로 승인하고 이메일 일치만으로
자동 병합하지 않는다. 현재는 external identity 테이블·OIDC 경로를 추가하지 않는다.

## DB 구현과 후속 설계

신원 PK/FK는 UUID, 시각은 timestamptz, 로그 ID는 bigint identity를 사용한다.
감사의 resource_id는 신원 UUID와 기존 Storage/Client slug를 담는 text다.
accounts/users/agents/credentials/sessions/audit_events는 `0008`에 구현했다.
command_invocations/security_events와 공통 로그인 예산은 후속 단계다.

| 테이블 (`management.*`) | 주요 컬럼 |
|---|---|
| `accounts` | id, kind(user/agent), display_name, role, is_active, deleted_at, created_at, updated_at |
| `users` | account_id PK/FK |
| `agents` | account_id PK/FK, owner_user_id FK(users) |
| `credentials` | id, account_id FK, label, token_prefix, token_hash UNIQUE, hash_version, created_at, expires_at, revoked_at, last_used_at |
| `sessions` | id, session_hash UNIQUE, auth_method, user_id FK, credential_id FK, master_generation, created_at, expires_at, revoked_at |
| `audit_events` | id, created_at, actor context, request_id, surface, action, resource_type, resource_id, metadata |
| `command_invocations` (후속) | id, created_at, actor context, request_id, surface, operation, outcome, error_code, duration_ms, redacted_metadata |
| `security_events` (후속) | id, created_at, nullable actor context, request_id, surface, event_type, outcome, reason_code |

```text
accounts
├─ kind=user  → users(account_id PK/FK)
├─ kind=agent → agents(account_id PK/FK, owner_user_id → users)
└─ 1:N       → credentials(account_id FK)
users + credentials ──1:N── sessions

master (config) ──설정 세대 검증── setup/recovery sessions
audit / invocation / security ── actor snapshot + request_id
```

| 제약 | 검증할 내용 |
|---|---|
| subtype | users는 kind=user, agents는 kind=agent; FK와 함께 종류·배타성 보장 |
| 소유 | Agent owner는 User; 초기에는 소유자 변경 없이 운영하고, User 삭제 시 소유 Agent도 무효화 |
| 세션 | token 방식은 user_id와 같은 User의 credential 필수; master 방식은 둘 다 NULL이며 master_generation 필수 |
| credential | 서버가 생성한 고엔트로피 값의 검증용 해시; 만료·폐기·소유자 상태를 매 요청 확인 |
| actor context | actor_kind(user/agent/master/anonymous/system), actor_id, owner_user_id, credential_id, session_id |
| 이력 보존 | actor/target은 snapshot ID; 계정·키 삭제와 독립적으로 보존 |
| index | credential 소유자, Agent owner, 세션 폐기 조회, 로그의 시각+ID·actor·target |

기존 로그인 제한은 DB 공유 예산으로 이관한다. 공개 경로의 rate limit과 함께 적용하고
제한 상태의 크기도 통제한다. 현행 `admin_*` 는 추가 migration으로 이관하며,
`storages/clients/client_keys/s3_credentials/files/locations` 의 소유 계약은 유지한다.

### 현재 DB 기반

| 구현 | 보장·경계 |
|---|---|
| subtype FK | accounts의 kind별 generated ID + deferred FK로 commit 시 정확히 한 subtype 보장 |
| 최초 설정 | 계정·첫 credential·감사 함께 commit; 기존 management 계정이 있으면 초기화 거부 |
| 변경 직렬화 | bootstrap·계정 변경·발급·복구·폐기·로그인이 같은 transaction advisory lock 사용 |
| 마지막 Admin | 활성 Admin의 강등·비활성화·삭제를 현재 DB 상태로 검사; 데이터 경로는 잠금 공유 없이 유지 |
| credential | 해시 v1은 소문자 hex 64자·UNIQUE; 원문 생성·해시 계산·TTL 결정은 후속 서비스 책임 |
| 현재 신원 | 매 인증 조회에서 만료/폐기·계정·Agent 소유자 상태를 읽어 정책용 Caller 생성 |
| User 세션 | User와 credential 소유자 일치 FK; Agent 로그인 거부; TTL=min(원본 만료, 8시간), 활성 세션은 credential당 64개 |
| 비활성화·삭제 | 비활성화 시 세션 폐기; 삭제 시 자기/소유 Agent 키도 폐기; 삭제 계정 재활성화는 제공하지 않음 |
| 복구 | 지정한 활성 Admin의 기존 키·세션을 폐기하고 새 키 발급; 다른 User·Client 키는 보존 |
| 변경 감사 | 신원 변경·세션 생성/폐기/상한 회수와 같은 transaction; 감사 실패 시 전체 rollback; 재폐기·같은 값 변경은 중복 이벤트 생략 |
| 감사 내용 | actor/request/target snapshot; 계정 변경의 role·active 전후 값; 자유 형식 payload 대신 내부 필드만 기록, metadata 8 KiB 상한 |
| 보존 | 계정은 soft delete; audit ID에는 FK를 두지 않아 물리 제거와 독립; 로그 보존/purge와 조회 scope는 후속 |
| 기존 DB | 기존 데이터가 있는 0007 → 0008 upgrade와 재실행 검증; 기존 계정의 자동 권한 매핑은 수행하지 않음 |

`db::management`는 내부 저장소 연산이다. `AuditContext`는 권한 증명이 아니며
HTTP에서 역직렬화하지 않는다. 후속 서비스는 인증·권한·CSRF 검증과 신뢰할 수 있는
actor/request ID 구성을 수행한 뒤 호출한다. 현재 API에는 이 연산이 연결되지 않았다.
master 세션은 DDL 형태만 정의했고 발급·세대 검증은 후속이다. last_used_at 갱신,
신원 목록·관리 이력의 범위 조회, 자원 변경과 새 audit의 결합도 후속이다.

## CLI·MCP의 공통 계약

명령명 19개·protocol 1·입출력·오류·권한 매핑은 [spec 09](09-management-commands.md)에
구현했다. CLI는 공통 DTO와 명령명을 재사용하며 기존 REST를 호출한다.
아래 공통 서버 실행기와 MCP adapter 연결은 후속 단계다.

```text
CLI adapter ──┐
              ├─ 공통 command schema → validation → authorization → management service
MCP adapter ──┘                                                      ↑
Console resource API ─────────────────────────────────────────────────┘
Console identity/history API ── User session + role → identity/history service
```

| 공통 항목 | 계약 |
|---|---|
| 대상 | storage·client·client-key·S3 credential·usage/status; 현재 CLI의 원격 기능 |
| 명령 정의 | 입력·출력·안정적인 error code·권한을 하나의 정의에서 제공 |
| transport | CLI는 JSON HTTP, MCP는 tool 호출; 각각 서버의 공통 명령 실행기를 호출 |
| protocol | package version과 별도의 호환성 식별자; 비호환은 실행 전에 거부 |
| 인증 | 같은 User/Agent 토큰에 같은 허용 결과; 기계용 경로는 master·Cookie 거부 |
| 부수 효과 | transport는 호출 기록, service는 변경 감사를 한 번만 기록 |
| 로컬 처리 | CLI의 update·설정·확인·비밀 파일 입출력은 adapter의 책임 |
| Native key | CLI의 raw key 파일→hash 변환 유지; 공통 명령은 현재의 hash 등록 계약 |
| 비밀 응답 | 의도한 서비스 키 발급 응답과 원문을 제외한 저장용 로그를 분리 |
| 재시도 | 변경 응답 불명은 unknown; 명시적 대조·재발급 절차 유지 |

User·Agent·role·관리 credential·감사 검색은 콘솔 전용이다.
Admin Bearer의 직접 호출도 identity/history API에서 거부한다.
MCP tool명·HTTP path·전송 envelope는 adapter 구현 전에 고정하고 동일 fixture로 검증한다.

## 관리 로그의 경계

| stream | 기록 대상 | 보장 |
|---|---|---|
| audit_events | User/Agent/role/token/session 관리, Storage/Client/서비스 키의 확정 변경, master 설정·복구 | DB 변경과 같은 transaction; insert 실패면 변경도 rollback |
| command_invocations | 인증 후 관리 API·CLI·MCP 요청의 성공/실패/조회 | best-effort; 기록 실패 시 완료된 결과 유지 |
| security_events | 인증 성공/실패·권한 거부·master 사용 | 인증 전은 actor 불명 허용; 저장 장애는 운영 로그로 알림 |

```text
request_id=R: MCP agent → storage.replace
  command_invocations: actor / credential / surface=mcp / outcome / duration
  audit_events:        actor / Storage ID / 허용된 변경 전후 / request_id=R
```

surface는 서버 adapter에서 확정하며 공식 CLI 실행 파일의 증명과 구분한다.
request_id는 서버가 발급하고, 외부 correlation 값은 검증된 별도 메타데이터로 취급한다.
인증 후 권한 거부는 호출 실패와 security event로 연결하며, 변경 audit는 생성하지 않는다.
인증 전 실패·잘못된 envelope·서버 도달 전 실패는 command 이력 보장과 구분해 처리한다.
외부 S3 접근 검증은 DB transaction과 한 번에 rollback할 수 없다. 확정된 등록 변경을
audit하며, 외부 효과가 남는 작업은 별도 작업 상태 계약으로 시작·결과·복구를 기록한다.

| 저장 규칙 | 내용 |
|---|---|
| payload | operation별 allowlist·크기 상한·안정 코드; 허용된 비밀 아닌 필드의 전후 값 |
| secret | 관리/서비스 token·S3 secret·Cookie·암호문·presigned URL·자유 형식 오류 원문 제외 |
| 관리 경계 | Client의 upload/download/delete·물리 GC는 별도 데이터/운영 로그 영역 |
| 외부 경계 | OAuth2 Proxy 거부·presigned 직접 전송은 각각의 로그로 관찰; Grove 도달 여부와 구분 |
| 조회·보존 | UI에서 상관 표시, 개별 수정/삭제 API 없이 보존 기간·bounded purge로 운영 |
| 강도 | 운영 감사의 append-only 계약; DB 관리자에 대한 변조 방지는 별도 요구 |

## 구현 순서와 완료 조건

| 단계 | 변경 | 검증 |
|---|---|---|
| 1a (로컬 구현·검증) | `management-policy`: 순수 신원·권한 규칙 | 역할/표면/인증 상태, Agent 상한, 감사 조회 scope, master 제한; 14개 테스트 |
| 1b (로컬 구현·검증) | `management-command`: 입력·출력·오류·schema·권한 매핑; CLI DTO 재사용 | 원격 19개 명령과 대응; identity/history 제외; 기존 CLI 회귀 테스트 |
| 2a (로컬 구현·검증) | `0008` + `db::management`: 신원·credential·세션·변경 감사의 저장소 기반 | 첫/마지막 Admin 경합, 발급/폐기 경합, 감사 실패 rollback, subtype/세션 FK, 기존 DB upgrade; 19개 PG 테스트 |
| 2b (다음) | 관리 서비스의 권한 연결·신원/이력 scope 조회·호출/보안 로그·로그인 예산 | 저장소를 호출하는 모든 서비스 경계의 정책, 자기/소유 Agent 범위, 비밀 제외, 로그 실패 구분 |
| 3 | master/User 로그인·identity API | CSRF, Agent 로그인 거부, token/session 폐기, 복구, master 세대 불일치 |
| 4 | 공통 resource command + CLI/MCP adapter | 동일 입력·결과·거부·409·unknown; audit 한 번, secret 로그 제외 |
| 5 | 콘솔 User/Agent/role/token/history | 역할별 표시·API 거부, 원문 한 번 표시, 응답 불명, light/dark·phone/tablet/desktop |
| 6 | 이관·proxy·기존 소비자 | DB backup, 이전 인증 종료, 복구 절차, Bearer/SigV4 보존, Native/S3 실제 전송 |

기존 관리 token은 사람·Agent·role의 대상 매핑을 명시적으로 승인한 뒤 전환한다.
전체 replica의 인증 지원을 맞추고 이전 token/session 종료와 이관을 직렬화한다.
이전 audit는 이전 방식의 기록으로 보존하며, 새 확정 변경 이벤트와 구분한다.
각 단계는 코드·테스트·대응 spec을 함께 커밋하고 공개·운영 전환은 별도로 수행한다.

구현 전에 고정할 값: 새 master 설정명·설정 세대 동기화 방식, token/session TTL·개수 상한,
로그 보존 기간·최대 payload·접근 예산, endpoint/tool명·전송 envelope, 이전 인증의 전환/복구 절차.
