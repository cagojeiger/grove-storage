# 소스 구조

```text
backend/crates/
├── management-service/   신원·이력 정책 실행 (User/master 인증 HTTP 연결)
│   ├── src/command.rs    콘솔 내부 명령 → Action·이름공간
│   ├── src/lib.rs        잠금·현재 신원·권한·server request ID
│   ├── src/dispatch.rs   허용 scope → 저장소 연산
│   ├── src/logging.rs    bounded best-effort 호출·보안 기록
│   ├── src/sessions.rs   로그인 예산·User 토큰 교환·인증 이력
│   ├── src/master.rs     설정 세대·master 세션·일회성 설정/복구 실행
│   ├── src/resources/   공통 권한·commit·안정 오류; reads.rs / writes.rs / storage.rs
│   └── tests/           authorization·history·logging·master_fencing·master_recovery·resources/·resource_writes/·storage_writes/
├── management-command/   자원 명령 19개·schema·권한 매핑 (조회 10개·변경 9개 연결)
│   ├── src/catalog.rs    명령명·입출력·권한·변경 여부의 정본
│   ├── src/input.rs      공통 입력·순수 값 검증
│   ├── src/model.rs      CLI가 재사용하는 응답 DTO
│   ├── src/error.rs      안정 오류 코드·변경 적용 여부
│   └── tests/            catalog·inputs·outputs·errors (DB·서버 독립)
├── management-policy/    관리 User·Agent·role·진입 경계의 순수 권한 규칙
│   ├── src/identity.rs   주체·역할·계정/자격증명 상태·인증 방식·Surface
│   ├── src/policy.rs     Action → 허용 Scope 또는 거부 사유
│   └── tests/           resources·console·agents·authentication (DB·서버 독립)
├── s3-protocol/           S3 XML·SigV4 순수 프로토콜 계약
│   ├── src/multipart.rs  Complete XML 구조·엔티티·namespace 검증
│   ├── src/completion.rs 완료 목록·원장 ETag·비최종 part 최소 크기 검증
│   ├── src/signing.rs    서명 계산·raw query 정렬
│   ├── src/auth.rs       scope·서명 헤더·만료 범위·본문 해시 검증
│   ├── src/integrity.rs  checksum 비교·읽기 If-Match·조건 헤더 식별
│   ├── src/operation.rs  지원 동작 분류·미지원 요청 차단
│   └── tests/            multipart·completion·signing·operation·auth (서버·DB 독립)
├── object-service/        grove-object-service: 업로드 준비·실패 보상 조율
│   ├── src/cleanup.rs     정리 성공 후 메타데이터 확정
│   ├── src/multipart_create.rs  vendor 생성·ID 기록·relay 준비·실패 보상
│   └── tests/            cleanup·cleanup_failures·multipart_create
├── object-policy/         grove-object-policy: 업로드 선언·파트·ETag·완료 복구 판단
│   └── tests/             geometry·etag·validation·completion·completion_failures
├── cli/                   gscli: User/Agent 공통 관리 명령 HTTP
│   ├── src/               인자·설정·HTTP·입력·확인·비밀 출력·응답 출력
│   │   ├── commands/      typed 명령·공개 결과·비밀 파일 전달
│   │   ├── http/wire.rs   protocol·명령·대상 대조·변경 결과 검증
│   │   └── update/        업데이트 흐름·다운로드·설치 기록·파일 교체, 분리된 tests/
│   └── tests/             설정·조회·변경·비밀·실패·status·update 테스트
├── api/src/
│   ├── mcp/              stateless HTTP·19개 공통 tool·실행기 연결
│   │   ├── contract.rs  command schema·도구 설명·annotation
│   │   ├── server.rs    SDK handler → 공통 실행기, surface=mcp
│   │   └── tests/       catalog·경계·권한·결과 대조·실패
│   ├── logging.rs       운영 subscriber·SDK payload 로그 차단
│   ├── resource_commands.rs  공통 자원 Bearer HTTP·envelope·서버 Surface
│   ├── resource_commands/tests/  기존 REST 비교·인증/입력·변경·감사/commit 장애
│   ├── console_identity/  인증·신원·이력 HTTP + 세션 전용 자원 adapter
│   │   ├── browser.rs    Origin·CSRF·cookie·Bearer 분리
│   │   ├── secrets.rs    token/session 형식·해시 domain
│   │   ├── session.rs    로그인·현재 세션·로그아웃 adapter
│   │   ├── accounts.rs   User/Agent 생성·role/active·삭제 adapter
│   │   ├── credentials.rs 관리 토큰 목록·일회성 발급·폐기 adapter
│   │   ├── history.rs    자기 세션·scoped 이력 adapter
│   │   ├── inputs.rs     요청 형태·역할·페이지 입력 검증
│   │   ├── output.rs     응답 allowlist·bigint 문자열 cursor
│   │   ├── master.rs     master 로그인·첫 Admin 발급·대상 복구 adapter
│   │   ├── master_config.rs  설정 쌍·형식 검증 (비밀 원문 오류 제외)
│   │   └── tests/        browser·lifecycle·failures·master/·master_recovery·identity/
│   ├── admin/              기존 등록부 REST·usage
│   │   └── storages.rs     기존 REST 요청·응답·등록부 DB 연결
│   ├── s3/                 SigV4·라우팅·객체·multipart
│   │   ├── object_response.rs Range·응답 헤더 정책
│   │   ├── integrity.rs    실측값·HTTP 헤더 연결, checksum 오류 응답
│   │   └── object_response/ Range·응답 헤더 테스트
│   ├── v1/                 네이티브 파일·multipart·relay
│   │   └── multipart_create.rs  Native 생성 service의 DB·S3·crypto adapter
│   ├── blobs.rs            lease URL 바이트 전송
│   ├── spool.rs            스트림 계측·임시 파일
│   ├── spool/tests.rs      청크별 누적 해시·네이티브 계측 유지
│   ├── storage_access.rs   등록부에서 backend 구성·물리 작업
│   ├── storage_registration.rs  REST·공통 명령·부팅·status의 검증·접근 확인·암호화
│   ├── storage_registration/tests.rs  URL·기본값·FS 필드·부팅/status 계약
│   ├── status.rs           현재 로컬 DB·저장소 진단 CLI
│   └── reconciler/         완료 복구
│       └── reclaim.rs      만료 회수의 물리 정리·재시도
├── db/
│   ├── src/management/     관리 신원 저장소 (기존 admin_*와 분리)
│   │   ├── accounts.rs    최초 Admin·User/Agent 생성·마지막 Admin 보호
│   │   ├── credentials.rs 발급 한도·폐기·대상 Admin 복구
│   │   ├── sessions.rs    User 세션·원본 만료 상한·폐기·개수 상한
│   │   ├── master.rs      master 세대 fence·짧은 세션·설정/복구 transaction
│   │   ├── identity.rs    현재 DB 상태 → 정책용 Caller
│   │   ├── audit.rs       변경 transaction 내부 감사 기록
│   │   ├── transaction.rs 권한 검사와 변경이 공유하는 transaction
│   │   ├── resources.rs   신원 transaction 안에서 기존 registry/usage 조회 재사용
│   │   ├── resource_writes.rs Client/서비스 키 SQL 재사용·같은 transaction 감사
│   │   ├── storage_writes.rs Storage 변경·행 잠금·비밀 없는 전후 감사
│   │   ├── queries.rs     비밀 없는 신원 목록·페이지 상한
│   │   ├── history.rs     actor/owner scope·cursor 조회
│   │   └── telemetry.rs   호출·보안 기록·공유 로그인 예산
│   ├── src/files/          파일·lease 상태 전이
│   │   └── reclaim_cleanup.rs  reclaimed 정리 후보·확정
│   ├── src/s3_registry/    자격증명·논리키·업로드 세션
│   ├── migrations/         PostgreSQL 스키마
│   └── tests/              DB 통합 테스트
├── infra/src/              fs·외부 S3 I/O
└── core/src/               설정·암호·해시, multipart는 policy 재노출
```

## 책임

| 모듈 | 입력 → 결과 | 정합성 경계 |
|---|---|---|
| `management-policy` | 검증된 Caller snapshot·Surface·Action → Scope/거부 | 순수 권한만 판정; 인증·scoped DB query·감사 transaction은 adapter/service 책임 |
| `management-service` | transport proof·Surface·내부 Command → 권한 검사·결과·호출 기록 | identity lock 이후 현재 신원 확인; console scope; User 로그인 예산·인증 이력 |
| `management-service/resources` | 공통 Command → 권한·DB 조회/변경·typed Output·호출 기록 | 19개 전체; Storage probe는 잠금 밖, 이후 role/owner·참조 재확인·audit 원자성 |
| `api/resource_commands` | CLI 등 Bearer·JSON → 공통 자원 실행기 | Cookie 거부·server Surface·명령/envelope 검증; 이전 REST 인증과 분리 |
| `api/mcp` | MCP Bearer·tool 입력 → 같은 자원 실행기 | 요청별 인증·Origin/Cookie 차단·공통 schema·surface=mcp; 권한 cache 없음 |
| `api/console_identity` | HTTP token/cookie·입력 → 신원/이력 service; `resources.rs` → 공통 자원 실행기 | Origin/CSRF·별도 쿠키·해시 domain·비밀 없는 목록; 현재 UI는 User 세션 사용 |
| `management-command` | protocol·명령명·JSON → typed 명령/오류; JSON → typed 출력 | 입력 형태·값·schema·권한 매핑; 서비스 검증·실행·감사·전송은 별도 책임 |
| `db/management` | 인증/권한 검증 후 내부 요청 → 신원 변경 + 감사 commit | 단일 identity lock·FK·감사 rollback; HTTP 인증/CSRF·정책 허용과 구분 |
| `object-service/cleanup` | 물리 정리 → 조건부 DB 확정 | 정리 실패 시 DB 작업 호출 생략; 원자성은 DB 소유 |
| `object-service/multipart_create` | 예약된 업로드 → vendor·relay 준비 | 실패 시 알려진 upload ID로 보상, 원래 오류 유지 |
| `api/routes`, `api/admin` | HTTP → 인증된 요청 | 표면별 인증·예약 경로 |
| `api/storage_registration` | 제출 필드 → 접근 확인·암호화된 행; 등록부 → 접근 상태 | REST·HTTP 명령·MCP·부팅·status가 공유; DB 변경·감사·권한은 호출부 소유 |
| `api/s3/auth` | 원본 URI·헤더 → client | SigV4 검증 |
| `api/s3/object_response` | Range·쿼리 → 응답 정책 | 인코딩·헤더 검증 |
| `api/s3/handlers`, `multipart` | 인증된 요청 → 저장·확정 | DB 소유권 → 물리 I/O → DB 확정 |
| `db/files`, `db/s3_registry/uploads` | 전이 요청 → 조건부 결과 | 행 락·트랜잭션 |
| `db/s3_registry/keys` | 논리키 교체 → 옛 파일 detach | 호출자의 확정 트랜잭션에 참여 |
| `infra/fs`, `infra/s3` | 물리 주소 → 바이트 I/O | filesystem·vendor 계약 |
| `api/reconciler` | DB 후보·실물 관찰 → 복구 | 보존된 소유권·재시도 |
| `api/status` | 로컬 Config → DB·저장소 접근·요약 | HTTP 독립, 부팅과 같은 storage 검사 |
| `cli` | User/Agent 토큰·인자 → 공통 명령 HTTP → table·JSON | DB 의존성 없음; wire 검증·변경 결과·비밀 파일 관리; 로컬 status와 분리 |
| `cli/update` | 공식 Release → 검증된 실행 파일 | 서버 인증 독립, 설치·업데이트의 동일 잠금·교체 |
| `object-policy` | 값 → 업로드 검증·파트 계산·ETag·복구 결정 | HTTP·DB·런타임·환경 설정 독립 |
| `core` | 환경 설정·암호·키 해시 | 기존 multipart import 경로는 policy 재노출 |

`uploads`의 크기보다 상태 전이의 원자성을 우선한다. 논리키 교체와 옛 파일
detach는 같은 트랜잭션을 공유한다.

## Naming And Priorities

| Name | Responsibility |
|---|---|
| Grove Storage / `gscli` | Product / remote management CLI |
| User / Agent | Human operator / automation identity owned by a User |
| Client | Runtime consumer with file API keys or S3 credentials |
| Storage | Registered filesystem or external S3 backend |
| Admin / Operator / Viewer | Management roles, separate from runtime Client credentials |
| `management-*` | Operator identities, permissions and resource commands |
| `object-*` | File lifecycle decisions and execution ordering |
| `s3-protocol` | S3 wire contracts |
| `api`, `db`, `infra`, `core` | Transport composition, persistence, provider I/O, configuration/crypto |
| `api/admin`, `api/admin_auth`, `admin_*` tables | Legacy management compatibility |
| `filegate-*`, `filegate`, `FILEGATE_*` | Retained package, executable and configuration names |

```text
Legacy REST ---------+
Resource HTTP / MCP -+--> storage_registration --> infra / crypto
Console via HTTP ---+          |
Startup / status ---+          +--> registry reads for health checks

Resource commands --> management-service --> db + transactional audit
Runtime file/S3 --> object-policy / object-service --> db / infra
```

| Order | Change | Acceptance |
|---|---|---|
| 1 - implemented | Move shared registration out of legacy REST | Same validation, probes, encryption and response contracts; 11 crates retained |
| 2 - next | Bind frontend command names to input/output types | Type errors catch mismatched calls; existing runtime response validation remains |
| 3 - after migration verification | Retire superseded legacy management entry points | Operator migration, rollback and explicit removal scope verified first |
| 4 - separate compatibility change | Align remaining Filegate package/configuration names | Release, installer, image, deployment and configuration migration checked together |

Use a module for a shared responsibility inside one executable. Keep an existing
crate when it provides an independently tested contract or cross-surface reuse.
Split tests by scenario; keep lock and transaction ownership together.

## 검증 위치

| 범위 | 테스트 |
|---|---|
| 관리 권한·콘솔 전용 경계·Agent 상한·감사 조회 scope | `cargo test -p grove-management-policy --locked`; 실제 API 연결과 구분 |
| 관리 명령·입출력·오류·CLI 대응 | `cargo test -p grove-management-command --locked`; `cli/tests/command_contract.rs` |
| MCP 실제 전송·CLI 대응 | `api/src/mcp/tests/`, `scripts/e2e-mcp.py`; PG/HTTP·19개 lifecycle·서버 로그·owner/폐기 |
| 신원 DB·변경 감사·이관 | `db/tests/management_{accounts,credentials,credential_limits,sessions,schema,upgrade}.rs`; 실제 PostgreSQL 필요 |
| 관리 서비스 권한·범위·로그 장애 | `cargo test -p grove-management-service --locked`; PostgreSQL의 `DATABASE_URL` 필요 |
| User/master·신원/이력 HTTP·브라우저 요청 경계 | `cargo test -p filegate-api console_identity --locked`; 실제 PostgreSQL, 브라우저 E2E와 구분 |
| S3 XML·서명 계산 | `cargo test -p grove-s3-protocol --locked` |
| S3 SDK·실제 HTTP 계약 | `scripts/e2e-s3.py --backend fs|minio` (boto3, 격리 DB·서버); MinIO 수명·중지/복구는 `s3_backend_fixture.py` |
| S3 완료 응답 유실 | `scripts/e2e-s3-recovery.py`; `s3_fault_proxy.py`가 MinIO Complete 응답을 끊고 실제 Reconciler 복구 확인 |
| 응답 유실 후 프로세스 재시작 | 같은 스크립트의 `--restart`; SIGKILL·새 PID·동일 DB로 복구 확인 |
| DB 커밋 거부 후 복구 | 같은 스크립트의 `--db-failure`; `s3_db_fault.py`가 격리 DB의 deferred trigger로 커밋 실패 주입 |
| 실제 콘솔 | `frontend/web/src`: app·auth·api·design·features/overview |
| 콘솔 테스트 | `frontend/web/tests`: 전송 단위·mock UI·실제 HTTPS 세션; `scripts/e2e-console.py`가 격리 환경 구성 |
| 정리 실행 순서·실패·재시도 | `object-service/tests/{cleanup,cleanup_failures}.rs`; `cargo test -p grove-object-service --locked` |
| 순수 업로드 규칙 | `object-policy/tests/{geometry,etag,validation}.rs`; `cargo test -p grove-object-policy --locked` |
| 완료 복구 판단·관찰 실패 | `object-policy/tests/{completion,completion_failures}.rs` |
| 조합 라우팅·인증·CORS | `api/src/routes/tests.rs` |
| 공통 Storage 등록·부팅/status·기존 REST 변경 호환 | `api/src/storage_registration/tests.rs`; `api/src/resource_commands/tests/{storages,storage_s3,storage_failures}.rs` |
| S3 서명·쿼리 | `s3-protocol/tests/auth.rs`, `api/src/s3/auth/tests.rs`, `s3/mod.rs`; 실제 요청은 `scripts/s3_auth_cases.py` |
| Range·응답 헤더 | `api/src/s3/object_response/tests.rs` |
| 파일 상태·동시성·GC | `db/tests/file_*`, `native_multipart_completion.rs` |
| S3 원자적 교체·완료·회수 | `db/tests/s3_*` |
| filesystem 조립·임시 보호 | `infra/src/fs.rs` |
| 현재 CLI 표현 | `api/src/status.rs` (바이트·용량 2개) |
| 원격 CLI 조회·상태·wire | `cli/tests/{config,reads,failures,status,wire}.rs`, `cli/src/output_tests.rs` |
| 원격 CLI 변경 | `cli/tests/{inputs,storage_writes,identity_writes,confirmations,secrets,mutation_failures}.rs` |
| CLI·서버 응답 계약 | `scripts/e2e-cli.py` (CI, 격리 DB·실제 서버) |
| CLI 설치·릴리스 계약 | `deploy/tests/test_{installer,manifest,version}.py` |
| 실제 바이트 경로 | `scripts/e2e-*.sh`, `scripts/s3-capture.py` |

실행 명령은 [기술·운영](../stack/README.md#검증)을 따른다.
