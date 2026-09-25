# spec 06: 관리 콘솔

- 상태: Storage·Clients·Accounts·Activity·Settings 로컬 구현, 미릴리스·미배포. 현재 샘플 미리보기는 `frontend/web/scripts/preview.mjs`이며 `output/`은 이전 시안이다.
- 선행 계약: [관리자 인증](05-admin-auth.md), [CLI](04-cli.md), [등록부](01-registry.md).
- 결정: 기존 관리 API를 공유하고 PostgreSQL을 정본으로 사용한다.
- 브라우저 배포·인증 완료 조건: [보안 경계](07-browser-security.md).

## Phase-One Scope

External S3-compatible storage is the supported backend. Console, CLI, MCP and
legacy REST accept only S3 registration/replacement. The console manages S3 client
credentials only. Native keys remain supported by the existing API, CLI and MCP.
Legacy FS rows remain visible with editing disabled; runtime I/O and
reference-protected deletion are retained pending inventory and migration.
Saved S3 storage supports an explicit on-demand connection test.

## Current Coverage

| Capability | Console UI | API / CLI / MCP |
|---|---|---|
| Account token login, Overview, Storage CRUD | Implemented | Implemented; browser sessions and Bearer remain separate |
| Root and setup/recovery | Implemented; protected config account | Separate Root and setup sessions; [ADR 011](../adr/011-root-and-accounts.md) |
| Accounts, management tokens | Implemented; Root/Admin only | Console-only identity API implemented |
| Clients, S3 credentials | Implemented; Reader views Clients, Writer/Admin/Root manage S3 credentials | Shared commands; Native key compatibility retained outside console UI |
| Sessions, management audit/call/security history | Implemented with cursor paging and event details | Console-only scoped APIs implemented |
| Daily usage snapshots | Implemented under Overview; 1-3650 days, default 90 | Shared `usage.history` |
| On-demand storage connection test | Implemented for saved S3 settings | Shared `storage.test`; `gscli status` still reads metadata only |

### Clients Contract

| View | Contract |
|---|---|
| List/detail | Client ID, assigned storage on detail, active-file usage; a successful empty usage result means zero active files |
| Create | Client ID and registered S3 storage; existing registry slug and uniqueness rules |
| Delete | Typed Client ID confirmation; server reference checks include files/uploads/cleanup, independently of active usage |
| Native compatibility | Existing API/CLI/MCP and DB behavior retained; no Native key controls in the console |
| S3 keys | `credential.create` returns a one-time access ID/secret; subsequent lists contain access IDs only |
| Revocation | Target key plus typed Client ID; existing shared command and audit transaction |
| Uncertain write | Stop resubmission, clear entered secrets, and require list review; no automatic mutation retries |

The sample preview returns in-memory data. Its successful storage forms are not
evidence of real provider connectivity. Real registration/replacement runs provider
checks before persistence. `readyz` and usage summaries are separate from those checks.
The server-local `filegate status` probes all registered backends, but is not a
remote per-storage command for Console/CLI/MCP.

## Target Navigation

Overview, Storage, Clients, Admin/Root-only Accounts, Activity and Settings are linked in `App.tsx`.
The signed-out entry links to Initial setup / recovery.

```text
Grove Storage
├── Overview
│   └── Usage history  daily snapshots by Storage and Client
├── Storage       list / detail / register / replace / delete / test connection
├── Clients       list / create / delete
│   └── Client detail
│       ├── Overview   assigned storage / usage
│       ├── Keys       S3 credentials
│       └── Logs       planned: runtime file requests / observed results
├── Accounts      Root / Users / management tokens (Admin and Root only)
├── Activity      scoped management history
│   ├── Audit     committed management changes
│   ├── Command history  Console / CLI / MCP management invocations
│   └── Security  authentication / authorization events
└── Settings      current account / role / own login sessions

Entry screens    Account token sign-in / Root setup / recovery
Header           theme / sign out
```

User identifies the management account; credential_id identifies each token. Client identifies a runtime consumer;
its Native/S3 keys and file access logs remain separate from management tokens/audit.

| Section | Boundary |
|---|---|
| Clients | App identity, assigned storage, file ownership, usage and service keys; Logs is runtime file history |
| Accounts | Protected Root, Users, roles, management token issuance/revocation; separate from future Storage Node agents |
| Activity | Audit / Calls / Security under existing actor/owner scopes; classify by operation, not transport |
| Settings | Current account and role, cursor-paged own browser sessions, explicit revocation; current-session revocation signs out |

Client detail can link to Activity filtered by Client ID, reusing the same management
audit records. Server-side Client filtering is pending; filtering one fetched page
in the browser is not a complete history. Client Logs is a separate future view;
its query API, observation coverage and retention contract remain pending, with the
limits described in [management logs](08-management-plane.md#client-history-coverage).
Settings uses the existing own-session API. Revoking a session leaves its account
token valid; revoking a token is an Accounts action. Activity shows installation
history to Admin/Root and own history to Reader/Writer; security events require
Admin/Root. Event details include actor, token/session IDs, surface and request ID.
Neither screen writes deployment environment variables.

## Usage History

| Item | Contract |
|---|---|
| Entry | Overview → Usage history (`#usage`); Reader/Writer/Admin/Root resource-read permission |
| Query | Shared `usage.history` with `days` 1-3650, default 90; no extra DB table or API |
| Rows | UTC snapshot date, Storage ID, Client ID, active files and active bytes; newest date first |
| Historical identity | Deleted Storage/Client IDs remain plain text in snapshots |
| Missing data | Empty period remains empty; missing dates are not fabricated as zero usage |
| Rendering | First 100 rows, then Show more; the API returns the entire requested window, not server-paged results |
| Meaning | Recorded stock snapshots, not file-request logs, transfer completion or exact historical accounting |
| Limits | Values outside JavaScript's safe integer range are rejected; unbounded row growth needs a future server paging contract |

## Connection Test

| Item | Current behavior |
|---|---|
| Entry points | **Test connection** in Storage detail; `gscli storage test ID`; MCP `storage.test` |
| Shared command | `storage.test` with `{id}` through the same Console, CLI and MCP executor |
| Permissions | ReadResources: Reader/Writer/Admin and console Root; identity rechecked after the probe |
| Saved storage | Resolve provider credentials on the server; return checks without secret values |
| Draft fields | Registration/replacement retain their existing pre-save probe; a standalone draft test is deferred |
| S3 baseline | Internal endpoint `HeadBucket` and `ListMultipartUploads`, matching current registration checks |
| Result | `{id,state:"ok"}` after both checks; 503 on provider/decryption/timeout failure; provider diagnostics stay private. UI shows local observation time |
| Scope | S3 baseline does not prove object PUT/GET/DELETE permission, public endpoint reachability or browser CORS |
| Execution | 10-second probe timeout outside the identity transaction; explicit click, one pending UI request, no automatic UI retry. No global concurrency quota yet |
| Freshness | Re-read saved settings after probe; changes return 409 and deleted targets return 404. UI clears results on detail refresh or navigation |
| Records | Management invocation result; registry metadata remains unchanged, so no storage-change audit event |
| Preview | Sample server returns unavailable; real provider checks are exercised with disposable MinIO |

## Next UI Priorities

| Order | Deliverable | Acceptance |
|---|---|---|
| 0 (Implemented) | S3-only admission | Console/API/CLI/MCP reject FS create/replace; legacy FS runtime retained. Inventory and migration/rollback remain before runtime removal |
| 1 (Implemented) | Root setup/recovery and Accounts | A new installation can issue its first User token; Admin can create User tokens; last-Admin and one-time-secret safeguards |
| 2 (Implemented) | Clients and service keys | Same lifecycle as CLI/MCP; reference-conflict protection, one-time S3 secret and unknown-outcome handling |
| 3 (Implemented, saved settings) | Shared connection test | Saved S3 probe via API, Console, CLI and MCP; failure, permission, timeout and concurrent setting/revocation tests |
| 4 (Implemented) | Activity and Settings | Scoped queries, cursor paging, event details, own-session revocation; server-side Client/date filtering remains follow-up |
| 5 | Client Logs | Defined observation/retention contract, scoped server queries and paging; URL issuance distinguished from transfer completion |

Frontend command typing is supporting work within these slices. Existing backend
identity/resource contracts are reused; new crates or a second authentication model
are not required for these screens.

| 항목 | 구현 계약 |
|---|---|
| UI | React·TypeScript·Vite, 기존 NoteGate 참고 기록의 semantic token·공통 UI 패턴 |
| 기본 언어 | 영어 문구·접근성 라벨, HTML `lang=en`, 숫자 `en-US`; 언어 선택기는 현재 범위 밖 |
| 데이터 | 같은 origin의 `/api/admin/console-commands/v1`; CLI/MCP와 공통 실행기·명령 계약 공유 |
| 인증 | 개인 `gsm_` 토큰 → `/api/admin/identity/v1/session` → HttpOnly User 쿠키; 원문 즉시 제거 |
| 변경 요청 | `X-Grove-CSRF: 1`, 서버의 Origin 검사 적용; POST 기반 조회 명령에도 적용 |
| 역할 | Reader는 조회, Writer/Admin은 자원 변경; 서버가 매 요청에 현재 권한 확인 |
| 브라우저 저장 | 테마 설정만 영속화, 토큰·세션·provider secret은 영속화 대상에서 제외 |
| 배포 경로 | 전용 관리 호스트의 `/api/admin/console/`에 정적 파일, `/api/admin/identity/v1`·`/api/admin/console-commands/v1`·`/readyz`만 서버로 전달 |
| HTTPS | 앞단 TLS 종료, `FILEGATE_CONSOLE_ORIGIN`과 실제 origin 일치 |
| 개발 | 동일 origin HTTPS 프록시 아래 UI·API 연결; Secure 쿠키 계약 유지 |

루트 `/{bucket}/{key}`는 별도 데이터 호스트의 S3 API가 사용한다. 콘솔은 이미 예약된
`api` 경로 아래에 배치한다. 관리 호스트는 파일·S3·relay 경로에 404를 반환한다.
정적 파일 배포·프록시 배선은 구현 시 검증한다.

## CLI 대응

원격 작업은 [공통 명령](09-management-commands.md)을 사용한다. 콘솔의 클라이언트 키는 S3로 한정한다.

| CLI 기능 | 화면 | API / 의미 |
|---|---|---|
| `status` | 개요 | readyz·등록부 조회 조합; 저장소 실물 점검과 구분 |
| `storage list/show` | 저장소 목록·상세 | `storage.list/show` |
| `storage create/replace/delete` | 등록·교체·삭제 | `storage.create/replace/delete` |
| `client list/show/create/delete` | 클라이언트 목록·상세·생성·삭제 | `client.*` |
| `client-key list/register/delete` | 콘솔 범위 밖 | 기존 Native API·CLI·MCP 호환 유지 |
| `credential list/create/delete` | S3 키 | `credential.*`; secret은 발급 응답에서 한 번 제공 |
| `usage storages/clients/history` | 개요·상세·Usage history | `usage.storages/clients/history` 사용 |
| `update` | 콘솔 범위 밖 | 사용자 PC의 바이너리 교체는 CLI의 로컬 기능 |

동등성 대상은 위 등록부 원격 작업이다. 현재 `filegate admin init/recover`는 운영자 로컬
명령이다. 신원·관리 토큰 관리·감사 조회는 콘솔 전용이며 CLI/MCP에는 제공하지 않는다.

## 로그인·토큰 관리 전환

[ADR 009](../adr/009-management-identity-and-command-boundary.md)의 개인 토큰 로그인·역할 표시·자원 연결을 구현했다.
최초 설정/복구·User·관리 토큰·본인 세션·관리 이력 UI를 구현했다.
권한·DB·전환 순서는 [관리 영역 설계](08-management-plane.md)를 따른다.

| Access flow | Current UI contract |
|---|---|
| Initial setup | Master token opens a limited setup session; first Admin token is shown once; User login is a separate step |
| Recovery | Known active Admin UUID and explicit confirmation replace that Admin's tokens and revoke its sessions |
| Accounts | Cursor-paged Users; create, change role, enable/disable and delete; named tokens share the User role |
| Tokens | Label, prefix, expiry and status; issue with 1–90 day expiry, copy once, acknowledge before closing; revoke with confirmation |
| Safety | Server enforces last-Admin and owner/role rules; disable/delete require the account name; unknown outcomes block resubmission |
| Paging | Search filters loaded accounts; Load more fetches the next server page |

```text
초기 설정 / 복구         master → 제한된 설정·복구 세션
로그인                  개인 관리 토큰 → User 세션
설정 / 접근 관리         Admin 전용
└── 사용자              생성 · 역할 · 활성 상태 · 용도별 토큰
내 세션                 조회 · 종료
활동 이력               관리 변경 / 관리 호출 / 보안 이벤트
```

토큰 목록은 이름·접두사·만료·상태를 표시한다. 발급 결과에서 원문을 한 번
표시하고 닫을 때 제거한다. User 토큰은 콘솔 로그인과 CLI/MCP에 사용한다. 용도별 label과 credential_id로 기록·폐기를 구분한다.
`gscli`·MCP는 같은 User 토큰을 전달하며 자원 작업만 제공한다. 신원·관리 토큰·감사 조회 API는
사람의 콘솔 세션과 역할로 보호한다. Admin Bearer도 이 경계를 대신하지 않는다.
Reader/Writer의 이력은 자기 범위, Admin은 전체 범위를 조회한다. 전체 보안 이력은 Admin 전용이다.
클라이언트 화면의 S3 키는 파일 서비스 자격증명으로 구분한다.

## 입력과 안전장치

| 영역 | 화면 계약 | 최종 집행 |
|---|---|---|
| S3 등록 | id, endpoint, public_endpoint, region, bucket, path-style, access key, secret, capacity, relay | 서버 필드 검증·접근 검증 |
| Legacy FS | Read-only detail; Edit disabled | Server rejects FS create/replace; delete retains reference guards |
| 등록 용량 | B/GiB/TiB 입력을 정수 bytes로 변환, JSON 정수 정밀도 상한 2^53-1 | 서버 i64 범위의 부분집합 |
| 저장소 교체 | `storage.replace` 전체 명세; secret 재입력, 기존 secret은 조회되지 않음 | location 존재 시 주소 변경 409 |
| 삭제 | 대상 ID 확인, 진행 중 중복 제출 차단, 409 시 이유와 최신 목록 표시 | DB 제약·서버 판단 |
| S3 키 발급 | 일회성 secret 표시·복사, 저장 확인 후 닫으면 제거 | 서버 발급·폐기 |
| 401 | 서버 데이터 캐시 제거 후 로그인으로 전환 | 세션 만료·폐기 확인 |
| 429 | Retry-After에 따라 재시도 안내 | 로그인 예산 |
| 변경 응답 유실 | 결과 미확정 표시, 목록 재조회·대조 | 자동 재발급·변경 재전송과 구분 |

목록의 파일 수는 삭제 가능성의 힌트다. 조회 후 동시 쓰기가 발생해도 서버의 409를
그대로 반영한다. 물리 저장소 삭제와 등록부 삭제는 구분한다.

## 구현 순서와 검증

| 단계 | 산출물 | 완료 기준 |
|---|---|---|
| A (구현) | 앱 골격·로그인·로그아웃·개요 조회 | 실제 HTTPS 쿠키 로그인, 새로고침 유지, 만료/폐기 401, 로그아웃, readyz·점유 표시 |
| B (구현) | 저장소 조회·등록·교체·삭제 | 실제 MinIO UI CRUD, 조회 후 참조 추가 409, 주소 교체 409, secret 미보관 |
| 5a (구현) | 개인 토큰 로그인·역할 표시·기존 자원 화면 전환 | 실제 HTTPS User 쿠키·폐기·Reader·역할 강등·토큰별 로그인·console 감사 |
| 5b (구현) | master 설정·복구·User·관리 토큰 UI | 실제 HTTPS 최초 설정·대상 복구·User 토큰 사용/폐기·마지막 Admin; 응답 불명·중복 제출·권한 변경·반응형 |
| C (구현) | 클라이언트·S3 키 | 공통 명령 사용, 한 번 표시·폐기, 응답 유실 시 중복 발급 방지 |
| 연결 검사 | 공통 `storage.test`와 상세 버튼 | 등록된 S3의 읽기 전용 probe, 권한·timeout·비밀 보호 |
| 관리 이력·세션 (구현) | 관리 변경·호출·보안 조회, 본인 세션 종료 | 범위별 조회·커서·상세, 독립 쿠키 세션 종료; 주체/대상/기간 필터는 후속 |
| D | 반응형·접근성·배포 | 320/390/768/1024/1440px, light/dark/system, 키보드·초점, 같은 origin 배포 |

각 단계는 단위 테스트·HTTP 통합·실제 브라우저 검증을 갖추고 커밋한다.
미리보기 테스트는 레이아웃 회귀 근거이며 실제 인증·API 연결의 증거와 구분한다.

```text
frontend/web/src/     현재 구현
├── app/              라우팅·초기화
├── api/              HTTP·오류·응답 타입
├── auth/             세션·로그인·master 설정/복구·401 캐시 제거
├── design/           테마·모달·용량 표시
└── features/
    ├── access/       User·역할·관리 토큰·일회성 비밀 표시
    ├── activity/     관리 변경·호출·보안 이력·상세
    ├── clients/      클라이언트·S3 자격증명
    ├── overview/     개요·저장소 점유·일별 사용량 이력
    ├── settings/     본인 로그인 세션·종료
    └── storages/     목록·상세·폼·삭제·입력 변환
```

현재 셸은 `app/App.tsx`가 소유한다. 저장소 목록·상세와 Access Users 목록은 해시 경로를 사용한다.
Access 계정 선택은 컴포넌트 상태이며 새로고침하면 해당 탭의 목록으로 돌아간다.
등록·수정 입력은 폼에 두며 변경 요청의 secret은 query/mutation 캐시에 넣지 않는다.
응답 유실·계약 불일치·`unknown/applied` 오류는 재제출을 잠근 뒤 조회로 대조한다.
검증된 `not_applied` 오류는 변경 전 거부로 표시한다. 자동 변경 재전송은 없다.
실행·검증은 [콘솔 README](../../frontend/web/README.md)를 따른다.
개요는 저장소·클라이언트 수와 저장소별 점유, 일별 사용량 이력을 제공한다.
Client 파일 로그, 관리 이력 서버 필터, 운영 배포와 전체 UX 재설계는 후속이다.
