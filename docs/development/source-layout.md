# 소스 구조

기준: 2026-10-07 작업 트리. 리소스 검증 이력은 [마감 점검](refactor-checkpoint.md),
현재 관리 영역의 완료·미검증 범위는 [완성도 점검](management-review.md)을 따른다.

## 파일 트리

```text
backend/crates/
├── management-command/          공통 자원 명령·schema·입출력·안정 오류
│   └── src/catalog.rs          Console·CLI·MCP 명령 정본
├── management-policy/          Caller·Surface·Action -> Scope 순수 권한
├── management-service/
│   ├── src/command.rs          콘솔 전용 계정·토큰·세션·이력 명령
│   ├── src/dispatch.rs         현재 신원·권한에 맞는 DB 실행
│   ├── src/resources/          공통 자원 명령·provider probe·변경 조율
│   ├── src/sessions.rs         비밀번호 로그인·과거 토큰 세션 코드
│   ├── src/local_accounts.rs  최초 Admin·로컬 계정 복구
│   ├── src/password_setups.rs 일회성 초기 비밀번호 설정
│   ├── src/personal_tokens.rs 본인 관리 API 토큰
│   ├── src/password_changes.rs 비밀번호 변경·세션 무효화
│   ├── src/logging.rs          bounded best-effort 호출·보안 기록
│   └── src/retention.rs        관리 로그의 기간별 유계 정리·실패 격리
├── s3-protocol/                 SigV4·XML·작업 분류·완료 목록·무결성 규칙
├── object-policy/               업로드 검증·파트 계산·ETag·완료 복구 판단
├── object-service/
│   ├── src/cleanup.rs          물리 정리 후 메타데이터 확정
│   ├── src/multipart_create.rs 생성·ID 기록·실패 보상
│   ├── src/single_commit.rs    Native 단일 업로드 완료 조율
│   └── src/multipart_commit.rs Native multipart 완료 조율
├── api/src/
│   ├── routes.rs              표면 조립·요청 제한·공통 telemetry
│   ├── routes/management.rs   계정·등록부 관리 HTTP/MCP 경로
│   ├── routes/objects.rs      Client/lease 인증 파일 경로
│   ├── accounts/              browser 인증·계정·토큰·세션·이력 adapter
│   ├── commands.rs            Bearer HTTP -> 공통 자원 실행기
│   ├── mcp/                   MCP -> 같은 자원 실행기
│   ├── storage_registration.rs 설정 검증·접근 확인·비밀 암호화
│   ├── storage_access.rs      등록 설정 -> S3 backend
│   ├── native/                Native HTTP·object-service adapter
│   ├── s3/                    S3 HTTP·인증·객체/multipart 작업 조율
│   ├── lease_relay.rs         lease URL 바이트 전송
│   ├── spool.rs               스트림 크기·해시 계측
│   ├── reconciler/            관찰·완료 복구·정리·사용량 스냅샷
│   ├── log_retention.rs       관리 로그 정리의 독립 주기·시간 제한
│   ├── shutdown.rs            HTTP·모든 작업·DB 종료의 공통 유예시간
│   └── status.rs              로컬 DB·등록 저장소 진단
├── db/
│   ├── src/management/        신원·설정 변경·감사 트랜잭션
│   │   ├── api_tokens.rs      Account 관리 API 토큰
│   │   └── retention.rs       만료된 관리 로그의 제한된 배치 삭제
│   ├── src/registry.rs        Storage·Client·서비스 키 등록 정보
│   ├── src/files/             파일·위치·lease 상태 전이
│   ├── src/s3_registry/       S3 자격증명·논리키·업로드 세션
│   ├── migrations/            새 설치 기준 SQL 5개
│   └── tests/                 PostgreSQL 정합성·경합·초기화·복원
├── infra/src/
│   ├── s3_io.rs               전송·물리 정리 조율, DB 상태 변경 없음
│   └── temp_spool.rs          전송용 임시 파일·정리
├── storage-provider/
│   └── src/s3.rs              S3 SDK·presign·제어 호출·바이트 I/O
├── core/                       환경 설정·암호·해시·기존 경로 재노출
└── cli/                        gscli HTTP·출력·비밀 전달·자체 업데이트
frontend/web/src/
├── template/                   출처·revision·라이선스가 고정된 MUI 공식 템플릿
│   ├── shared-theme/           공통 theme·시스템 폰트·입력/표시 스타일
│   ├── dashboard/              데스크톱/모바일 탐색·프레임·Data Grid/Charts
│   └── sign-in/                로그인 레이아웃
├── console/                    Grove 화면 조합·form·query·작업 연결
│   ├── Overview.tsx · ConnectionLines.tsx  연결 관계 시각화
│   ├── Resources.tsx · ResourceForms.tsx  Storage·Client·S3 키
│   ├── Accounts.tsx · AccountActions.tsx   관리자 계정 관리
│   ├── Profile.tsx · Tokens.tsx           본인 정보·인증·세션·토큰
│   └── Activity.tsx · UsageHistory.tsx    관리 이력·기록된 사용량
├── app/ · auth/ · api/          경로 상태·세션·HTTP/명령 계약
├── design/                     숫자·용량·시간 포맷
└── features/                   화면과 독립적인 query·검증·권한 모델
scripts/
├── e2e-cli.py                  격리 DB·실제 서버 CLI 검증
├── e2e-s3.py                   MinIO SDK 계약 검증
├── e2e-s3-recovery.py          응답 유실·재시작·커밋 거부
├── e2e-installation.py         새 DB 초기화·백업 복원·업로드 재개
├── e2e-notegate.py             실제 NoteGate 핸들러 연결
└── e2e-notegate-browser.py     NoteGate 서버·UI·로컬 OIDC·바이트 검증
```

`output/`은 과거 미리보기 산출물이며 실행 콘솔은 `frontend/web`이다.
트리는 주요 모듈만 표시한다. 전체 workspace 구성은 [Cargo.toml](../../Cargo.toml)이 정본이다.

## 책임과 계약

| 경계 | 소유 책임 | 유지할 조건 |
|---|---|---|
| Management | 계정·Storage·Client·키·metadata 설정과 감사 | 현재 신원 재확인·삭제/주소 변경 guard·감사와 변경의 원자성 |
| Object Service | 설정 조회·파일 수명주기·복구 조율 | 물리 작업 전후 상태·lease·정리 정보 보존 |
| Transfer / Provider | 외부 S3 제어 호출·바이트 이동 | DB·계정 의존성 없음; direct presign은 Client↔S3, relay·S3 endpoint는 Grove 경유 |
| DB | 락·상태 전이·논리키 교체·사용량 | 파일 확정과 옛 파일 detach를 같은 트랜잭션에서 처리 |
| HTTP/MCP | 입력·인증 envelope·응답 | Console cookie/CSRF와 CLI/MCP Bearer 분리 |
| Policy/Protocol | 순수 값·권한·프로토콜 판단 | DB·서버 없이 독립 테스트 |

서비스 추출은 실패 순서와 보상 경계를 기준으로 한다. S3 multipart의 I/O 조율 일부는
`api/s3/multipart.rs`에 남아 있고, Native 완료는 object-service를 사용한다.

```text
Account -> Management -> registry 설정·서비스 키 쓰기
Client  -> Object Service -> registry 읽기 + 파일·위치·lease 쓰기
                        -> Transfer -> 외부 S3
```

Resources 화면은 Management에 속한다. 파일 요청은 Client/lease로 인증하고
관리 Account·세션·역할을 조회하지 않는다. Management는 삭제·주소 변경의 안전성을
판단할 때 파일 참조 상태를 조회한다. 계정 폐기와 Client 서비스 키 폐기는 별개다.
현재 요청 경로는 API 프로세스·주 DB 풀·DB 크레이트를 공유하며, 관리 로그 정리만
전용 풀을 사용한다. 이 경계는 배포나 프로세스 장애 격리가 아니다.
Native JSON 경로의 본문/시간 제한과 S3·relay 스트리밍 경로의 제한은 별도로 유지한다.

## 의존성과 실행 경계

```text
api (composition)
  +-- management-service -> db / core / management-command / management-policy
  +-- object-service     -> std (operation ports, no DB or provider dependency)
  +-- infra              -> object-policy / storage-provider
  +-- storage-provider   -> core (clock) / AWS SDK

Background tasks (same process and database)
  +-- Object reconciliation -> main pool -> object advisory lock -> provider I/O
  +-- Management retention  -> dedicated pool (2) -> retention lock -> log pruning
```

S3 SDK를 `storage-provider`에 한정한다. `infra`의 재노출은 기존 내부 adapter import를
유지하기 위한 것이며 SDK 구현은 그 crate에 없다. Management의 연결 검사는 API가
주입한 verifier로 수행하고 관리 서비스가 provider에 직접 의존하지 않는다.
`scripts/check-crate-boundaries.py`는 production/build 의존성을 검사한다. dev-only
fixture는 제외하며 새 workspace crate와 금지된 의존성은 CI에서 실패한다.
이 검사는 crate 참조 경계이지 같은 DB 사용자에 대한 SQL 권한 격리는 아니다.

관리 로그 정리는 S3 작업과 다른 Tokio task와 advisory lock을 사용하며, 한 pass는
20초로 제한된다. 주기 값은 기존 reconciler 설정을 공유하지만 실행은 독립이다.
종료 시 JoinSet으로 두 작업을 함께 드레인하고 총 20초 초과 시 모두 취소한다.
관리 정리 전용 풀은 잠금 유지와 삭제 작업에 최대 2개 연결을 사용한다. 프로세스당
연결 상한은 설정된 주 풀 상한 + 2이며, 주 풀이 소진되어도 관리 정리는 대기하지 않는다.
DB 서버·CPU·메모리는 공유한다. Provider 전체 작업의 시간 제한과 복구 재시도 공정성은
이 구조 분리만으로 해결되지 않으며 별도 보완 대상이다.

## 이름과 저장

| 개념 | 현재 의미 |
|---|---|
| Account / Token | 관리 주체 / 이름 있는 관리 자격증명; DB `management.accounts`·`api_tokens` |
| 로컬 Admin | DB 계정; 서버 로컬 초기화·복구, 콘솔 비밀번호 로그인 |
| Admin / Writer / Reader | 관리 역할; Client 파일 접근 권한과 별개 |
| Client | Native 서비스 키·S3 자격증명으로 파일 API를 쓰는 소비자 |
| Storage | 외부 S3 저장소; 등록 용량·endpoint·provider 비밀·metadata |
| `root_path` | 응답 호환용 null; 새 DB에 열·FS backend 없음 |
| `temp_spool` | 전송 버퍼; 등록 가능한 저장소와 별개 |
| 서버 / CLI 이름 | `grove-storage`·`GROVE_*` / `gscli`·`GROVE_*`; crate는 `grove-*` |
| `master_configuration`, `root_sessions` | 새 DB에 생성하지 않음; 과거 actor label과 Agent-owner 필드도 없음 |
| `management.sessions.account_id` | 필수 Account FK; token은 동일 계정 credential FK, password는 generation 검사 |

명령 수는 [catalog.rs](../../backend/crates/management-command/src/catalog.rs)를 따른다.
계정/토큰 관리는 콘솔 전용 내부 Command이며 CLI/MCP 자원 catalog와 별개다.

## 검증 위치

경로는 `backend/crates/` 기준이다. 실행 결과와 환경은 [S3 호환성 점검](s3-compatibility-review.md),
기본 실행 명령은 [기술·운영](../stack/README.md#검증)을 따른다.

| 계약 | 테스트 위치 |
|---|---|
| 순수 규칙 | `s3-protocol/tests`, `object-policy/tests`, `management-policy/tests` |
| 완료·보상 순서 | `object-service/tests`의 single/multipart commit·cleanup·실패 테스트 |
| 관리 명령 schema·DTO | `management-command/tests` |
| 권한·감사 실패·신원 잠금 | `management-service/tests`, `db/tests/management_*` |
| 등록부 변경·가드 | `api/src/commands/tests`, `management-service/tests/resource_writes` |
| 등록 검증·S3-only | `api/src/storage_registration/tests.rs`, `db/tests/s3_only_schema.rs` |
| 새 DB·checksum·DDL rollback | `db/tests/schema_baseline.rs`, `scripts/e2e-installation.py` |
| 상태·GC·조건부 PUT·복구 | `db/tests/file_*`, `db/tests/s3_*`, `api/src/reconciler` |
| 브라우저 인증·표면 분리 | `api/src/accounts/tests`, `api/src/mcp/tests`, `api/src/routes/tests.rs` |
| 관리 DB와 파일 요청 독립성 | `api/src/routes/tests/independence.rs`: 격리 DB의 관리 스키마 제거 후 Native·S3 요청·소유 범위·서비스 키 폐기 검증 |
| Provider signing·multipart 조회 | `storage-provider/tests/time.rs`, `storage-provider/tests/multipart_cleanup.rs` |
| 임시 파일·계측 | `infra/src/temp_spool.rs`, `api/src/spool/tests.rs` |
| 의존성·독립 작업·종료 | `scripts/tests/test_crate_boundaries.py`, `management-service/tests/retention.rs`, `api/src/log_retention.rs`, `api/src/shutdown.rs` |
| CLI 출력·변경·업데이트 | `cli/tests`, `cli/src/update`, `scripts/e2e-cli.py` |
| UI | `frontend/web/tests`; fixture 화면과 실서버 검증을 구분 |

일반 E2E는 `e2e-cli.py`의 공통 fixture로 실행한다.
`grove-storage account init` → 서버 시작 → 비밀번호 로그인 → 관리 토큰 발급 → 자원 명령 순서이며,
구형 관리 API의 `410` 응답을 확인한다. 파일 요청은 별도의 Native·S3 서비스 키를
사용하며, 새 설치·복원 E2E는 Account 인증을 사용한다.

Management의 다음 변경 순서는 [완성도 점검](management-review.md)에 모은다.
