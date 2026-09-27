# 소스 구조

기준: S3-only 리소스 리팩토링 `520e172`. 완료·미검증 범위는 [마감 점검](refactor-checkpoint.md)을 따른다.

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
│   ├── src/root.rs · master.rs 공개 경로에서 제거된 과거 내부 코드
│   └── src/logging.rs          bounded best-effort 호출·보안 기록
├── s3-protocol/                 SigV4·XML·작업 분류·완료 목록·무결성 규칙
├── object-policy/               업로드 검증·파트 계산·ETag·완료 복구 판단
├── object-service/
│   ├── src/cleanup.rs          물리 정리 후 메타데이터 확정
│   ├── src/multipart_create.rs 생성·ID 기록·실패 보상
│   ├── src/single_commit.rs    Native 단일 업로드 완료 조율
│   └── src/multipart_commit.rs Native multipart 완료 조율
├── api/src/
│   ├── console_identity/      browser 인증·계정·토큰·세션·이력 adapter
│   ├── resource_commands.rs   Bearer HTTP -> 공통 자원 실행기
│   ├── mcp/                   MCP -> 같은 자원 실행기
│   ├── admin/                 기존 등록부 REST·usage 호환
│   ├── storage_registration.rs 설정 검증·접근 확인·비밀 암호화
│   ├── storage_access.rs      등록 설정 -> S3 backend
│   ├── v1/                    Native HTTP·object-service adapter
│   ├── s3/                    S3 HTTP·인증·객체/multipart 작업 조율
│   ├── blobs.rs               lease URL 바이트 전송
│   ├── spool.rs               스트림 크기·해시 계측
│   ├── reconciler/            관찰·완료 복구·정리·사용량 스냅샷
│   └── status.rs              로컬 DB·등록 저장소 진단
├── db/
│   ├── src/management/        신원·설정 변경·감사 트랜잭션
│   ├── src/registry.rs        Storage·Client·서비스 키 등록 정보
│   ├── src/files/             파일·위치·lease 상태 전이
│   ├── src/s3_registry/       S3 자격증명·논리키·업로드 세션
│   ├── migrations/            스키마 변경·업그레이드 전제
│   └── tests/                 실제 PostgreSQL 정합성·경합·업그레이드
├── infra/src/
│   ├── backend.rs             S3 backend 작업
│   ├── s3.rs                  vendor SDK·presign·바이트 I/O
│   └── temp_spool.rs          전송용 임시 파일·정리
├── core/                       환경 설정·암호·해시·기존 경로 재노출
└── cli/                        gscli HTTP·출력·비밀 전달·자체 업데이트
frontend/web/src/
├── app/ · auth/ · api/          탐색·로그인·HTTP 계약
└── features/
    ├── overview/ · storages/ · clients/ · metadata/
    └── access/ · activity/ · settings/   Accounts·이력·자기 세션
scripts/
├── e2e-cli.py                  격리 DB·실제 서버 CLI 검증
├── e2e-s3.py                   MinIO SDK 계약 검증
├── e2e-s3-recovery.py          응답 유실·재시작·커밋 거부
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
| Transfer | 외부 S3 바이트 이동·관찰 | direct presign은 Client↔S3, relay는 Grove 경유 |
| DB | 락·상태 전이·논리키 교체·사용량 | 파일 확정과 옛 파일 detach를 같은 트랜잭션에서 처리 |
| HTTP/MCP | 입력·인증 envelope·응답 | Console cookie/CSRF와 CLI/MCP Bearer 분리 |
| Policy/Protocol | 순수 값·권한·프로토콜 판단 | DB·서버 없이 독립 테스트 |

서비스 추출은 실패 순서와 보상 경계를 기준으로 한다. S3 multipart의 I/O 조율 일부는
`api/s3/multipart.rs`에 남아 있고, Native 완료는 object-service를 사용한다.

## 이름과 저장

| 개념 | 현재 의미 |
|---|---|
| Account / Token | 관리 주체 / 이름 있는 관리 자격증명; DB `management.accounts`·`credentials` |
| 로컬 Admin | DB 계정; 서버 로컬 초기화·복구, 콘솔 비밀번호 로그인 |
| Admin / Writer / Reader | 관리 역할; Client 파일 접근 권한과 별개 |
| Client | Native 서비스 키·S3 자격증명으로 파일 API를 쓰는 소비자 |
| Storage | 외부 S3 저장소; 등록 용량·endpoint·provider 비밀·metadata |
| `root_path` | 응답 호환용 null; migration 0017 이후 DB 열·FS backend 없음 |
| `temp_spool` | 전송 버퍼; 등록 가능한 저장소와 별개 |
| 서버 / CLI 이름 | `filegate`·`FILEGATE_*` / `gscli`·`GROVE_*` 유지 |
| `master`, `root_sessions` | 기존 데이터/내부 코드에 남은 이전 인증 구조; 공개 콘솔 진입 불가 |

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
| 등록부 변경·기존 REST 호환 | `api/src/resource_commands/tests/legacy_contract`, `management-service/tests/resource_writes` |
| 등록 검증·S3-only | `api/src/storage_registration/tests.rs`, `db/tests/s3_only_upgrade.rs` |
| 상태·GC·조건부 PUT·복구 | `db/tests/file_*`, `db/tests/s3_*`, `api/src/reconciler` |
| 브라우저 인증·표면 분리 | `api/src/console_identity/tests`, `api/src/mcp/tests`, `api/src/routes/tests.rs` |
| 임시 파일·계측 | `infra/src/temp_spool.rs`, `api/src/spool/tests.rs` |
| CLI 출력·변경·업데이트 | `cli/tests`, `cli/src/update`, `scripts/e2e-cli.py` |
| UI | `frontend/web/tests`; fixture 화면과 실서버 검증을 구분 |

Management의 다음 변경 순서는 [준비 계획](management-review.md)에 모은다.
