# Management 완성도 점검

구조 기준: 2026-10-07 작업 트리. 실행 결과는 각 절의 날짜·revision에 해당한다.
리소스 검증 이력은 [체크포인트](refactor-checkpoint.md), 인증의 상세 계약은
[로컬 관리 인증](../spec/11-local-management-auth.md)이 정본이다.

## 책임과 의존성

```text
Console cookie                   CLI / MCP Bearer
    |                                 |
    +--> 계정·비밀번호·토큰·세션·이력   |
    |    accounts                     |
    |       -> service::Command       |
    |                                 |
    +------> 공통 자원 명령 <----------+
             management-command
                     |
             management-service
             현재 신원 -> policy -> DB 변경 + audit
                     |
             registry + management

Client Native / S3 서비스 키
    -> Object Service -> registry 읽기 + 파일·위치·lease 쓰기
                      -> Transfer -> 외부 S3
```

Resources 화면의 등록·수정·삭제도 Management 책임이다. 파일 요청은 관리
Account·역할·브라우저 세션을 조회하지 않는다. 삭제·주소 변경 시 파일 참조를
검사하는 것은 등록부 변경의 안전 조건이다. 현재 요청 경로는 프로세스와 주 DB 풀을
공유하며 관리 로그 정리만 독립 작업·잠금·최대 2개 연결의 전용 풀을 사용한다.
이 구조는 책임 분리이며 별도 배포나 장애 격리를 제공하지 않는다.

## 현재 신원 구조

```text
management.accounts                 Admin / Writer / Reader
    +--> password_credentials       고유 login_name·Argon2id hash·generation
    +--> password_setup_tokens      일회성 설정 링크의 hash·만료
    +--> api_tokens                 관리 API 토큰의 hash·만료·폐기
    +--> sessions                   password generation 또는 원본 token 연결
    +--> authentication_budgets      login / reauthentication 요청 예산

변경 transaction ----> audit_events
요청 처리 후 --------> command_invocations
인증·권한 결과 ------> security_events
익명 로그인 예산 ----> login_budget
```

`0004_management.sql`이 현재 인증 테이블을 직접 생성한다.
계정과 자격증명은 권한·수명주기가 달라 별도 테이블을 유지한다.
Session의 `account_id`는 필수이며 Root/Master 인증 구조는 생성하지 않는다.
이 기준 스키마는 새 DB용이다. 기존 DB 업그레이드는 제공하지 않는다.
상세 계약은 [로컬 인증](../spec/11-local-management-auth.md), 검증 흐름은
[새 설치·복원](fresh-installation.md)을 따른다.

| 현재 동작 | 코드 근거 |
|---|---|
| 최초 Admin은 서버 로컬에서 대화형 비밀번호로 초기화 | `management-service/src/local_accounts.rs`, `db/src/management/passwords.rs` |
| 관리자가 계정을 생성하고 일회성 설정 링크를 전달 | `management-service/src/password_setups.rs`, `db/src/management/password_setup.rs` |
| 비밀번호 변경 시 generation 변경·계정 세션 폐기 | `management-service/src/password_changes.rs`, `db/src/management/passwords.rs` |
| 서버 로컬 복구 시 기존 역할 유지·세션과 관리 토큰 폐기 | `db/src/management/passwords.rs::recover` |
| 로그인·재인증은 hash 작업 전에 DB 예산 확인 | `db/src/management/admission.rs`, `management-service/src/resources/admission.rs` |
| 현재 신원과 역할은 직렬화된 변경 경계에서 재확인 | `management-service/src/lib.rs`, `db/src/management/transaction.rs` |
| 등록부 변경과 감사는 같은 transaction에서 확정 | `db/src/management/{resource_writes,storage_writes,resource_metadata}.rs` |

암호화 키·DB 접속 권한을 가진 서버 운영자와 콘솔의 Admin 역할은 다른 권한이다.
기본 비밀번호는 없다. 구형 운영자 REST는 명시적 호환 모드에서만 활성화한다.

## 로그의 책임

| 종류 | 질문 | 쓰기 계약 |
|---|---|---|
| Audit | 누가 어떤 관리 설정을 바꿨나? | 변경과 같은 transaction; 기록 실패 시 변경 롤백 |
| Command history | 어떤 관리 호출이 성공·실패했나? | best-effort, 각 쓰기 250 ms timeout |
| Security | 로그인·인증·권한 검사가 어떻게 끝났나? | best-effort, 각 쓰기 250 ms timeout |

Client 파일 전송 로그와 관리 감사는 별개다. 조회도 command invocation을 기록하며,
관리 로그는 Audit 365일·Security 90일·Command history 90일을 기본으로 보존한다.
서버 환경 설정과 워커의 테이블별 유계 배치 정리를 사용한다. 행 증가량과 운영 조회
비용은 측정 전이며 기간별 정책은 [운영 설명](../stack/README.md#관리-로그-보존)을 따른다.

## 완료 조건과 우선순위

| 순서 | 현재 근거 | 완료 조건 |
|---|---|---|
| 1. 브라우저 회귀 정리 | 현재 탐색·템플릿으로 검사 갱신; 전체 494개 통과 | 이번 작업 트리에서 확인; 권한·비밀·오류·복원 검사를 유지 |
| 2. 문서 정합성 | README·소스 구조·콘솔·브라우저 보안 문서 갱신 | 이번 범위의 현재/역사/계획 구분 완료; 후속 변경 시 함께 갱신 |
| 3. 운영 콘솔 제공 | 작업 트리의 Dockerfile은 서버와 콘솔 dist를 함께 포함 | 전용 HTTPS origin·응답별 CSP nonce·패키징 이미지 검증 |
| 4. 관리 이력 보존 | 기간 설정·테이블별 1,000건 배치·timeout·재시도 구현; 격리 PG 경계·배치·잠금·독립성 검증 통과 | 운영 증가량/정리 속도 측정; 외부 보관은 별도 |
| 5. 릴리스 준비 | 새 DB 설치·복원과 NoteGate 계약 검증이 CI에 존재 | 같은 후보 SHA의 CI·이미지·자산 검증; 운영 이관은 별도 |

서버를 추가하지 않으며 crate는 검증된 책임 경계에만 추출한다. S3 SDK는 현재
`storage-provider`에 격리했다. 템플릿의 기본 표시를 사용해도 기존 비밀 취급·권한·
오류 복구 계약을 약화시키지 않는다. 로컬 E2E 통과와 운영 배포 완료는 별도 상태다.

## 이번 점검의 실행 결과

2026-10-04 로컬 작업 트리 기준이다. 발행된 이미지나 홈 클러스터를 검증한 결과가 아니다.

| 검사 | 결과와 범위 |
|---|---|
| `npm run lint`, `npm run build` | 통과; TypeScript·Vite 빌드 포함 |
| `npm test -- --workers=2 --reporter=line,json` | 494개 전체 통과, 3.1분; light/dark·모바일/데스크톱·빌드된 CSP·권한·실패 상태 |
| `scripts/e2e-console.py` | 격리 PostgreSQL·Rust API·MinIO·HTTPS 브라우저 통과; 계정 생성/설정/복구·Reader·세션 폐기·CLI/MCP 토큰·metadata·presigned PUT/GET 바이트 일치 |
| 독립 Rust 계약 테스트 | `grove-management-policy`, `grove-management-command`, `grove-object-policy`, `grove-object-service`, `grove-s3-protocol` 통과; 전체 workspace 테스트 결과는 아님 |
| `cargo build --bin filegate --bin gscli --locked` | 로컬 개발 바이너리 빌드 통과 |
| `cargo fmt --all --check`, `git diff --check` | 통과 |
| `node --test scripts/preview-identity.test.mjs` | 1개 통과 |
| `deploy/ci/check-version.py`, `deploy/tests` | 버전 정합성 및 26개 통과; 설치 smoke는 로컬 `target/debug/gscli` 사용, 발행 자산 검증은 아님 |

브라우저 테스트를 삭제하거나 건너뛰어 맞추지 않았다. 제거된 상세 탭과 DOM/CSS
기대값을 현재 화면에 맞췄으며, 권한·중복 제출·불확실한 변경 결과·비밀값 검사는 유지한다.
회귀 점검에서 용량 0/초과 표시, Overview 새로고침 시 연결 정보 갱신, 비밀값 기본
마스킹, 사용량 이력의 90일 기본값과 1–3650일 범위, 기존 Storage 편집 링크를 보완했다.
이 정리에서 기존 파일 API나 DB 스키마를 추가 변경하지 않았다. 실제 서버 E2E의
임시 DB·MinIO·계정은 정리했으며 운영 전환은 수행하지 않았다.

### 관리 로그 보존 검증

2026-10-05 격리 PostgreSQL 17에서 검증했다. 만료 경계와 oldest-first 삭제 1개,
기간별 보존·계정/파일/키 독립성·배치 backlog·잠금 실패/재시도·공유 워커 락 4개가
통과했다. 설정 검사는 0·음수·잘못된 일수를 거부하고 기간 override를 확인한다.
`cargo test --workspace --locked`도 통과했다. 기존 CLI의 native release artifact
업데이트 테스트 1개는 `GSCLI_TEST_BINARY`가 없어 ignore 상태이며 발행 자산 검증은 아니다.
`cargo clippy --workspace --all-targets --locked -- -D warnings`, format·diff 검사도 통과했다.
운영 데이터와 홈 클러스터에는 적용하지 않았으며 실제 유입량/정리 성능은 측정 전이다.

### 배포 전 실패 처리 보완

2026-10-05 로컬 후보에서 다음을 추가 검증했다. 파일 API와 DB 스키마는 변경하지 않았다.

| 변경 | 검증 |
|---|---|
| `event-listener` 5.4.2·S3 SDK 1.152.0 및 호환 전이 의존성 업데이트 | `lru` 0.18.5; `cargo audit --deny unsound --ignore RUSTSEC-2023-0071` 통과 |
| multipart 정리의 marker 반복·순환·누락 및 1,000페이지 상한 | provider HTTP 회귀 6개; 정상 marker 이동·정확한 key만 중단하는 계약 포함 |
| HTTP·워커·DB 풀 종료의 총 20초 유예시간 | 종료 회귀 6개; 멈춘 실제 HTTP body·worker·pool close와 SQLx close 호출 순서 포함 |
| worker 취소 후 advisory lock 해제와 보존 정리 재개 | 격리 PostgreSQL에서 연결 풀 2개로 실행하는 회귀 1개 추가 |
| 실제 서버 SIGTERM | `scripts/e2e-shutdown.py` 통과; CI에 추가 |
| S3 SDK 실제 공급자 회귀 | 격리 MinIO SDK 계약·응답 유실·SIGKILL 재시작·DB commit 실패 복구 통과 |
| 실제 HTTPS 계정 회귀 | 생성·setup·Reader 권한·CSRF·두 세션·비밀번호 변경·재로그인·로그아웃 통과 |
| 최종 Rust workspace | 격리 PostgreSQL 17에서 전체 통과; native release artifact가 필요한 기존 CLI 검사 1개는 ignore |
| 추가 위생 검사 | Clippy 경고 없음·frontend production build·스크립트 단위 16개·배포 계약 26개 통과 |

Rust audit에는 `spin` 0.9.8·0.10.0의 yanked 경고 2개가 남아 있다. 메모리 안전성
경고는 해소했고 CI에서는 새 `unsound` 경고를 실패로 처리한다. RSA 예외는 기존의
비활성 `sqlx-mysql` 전이 의존성 근거를 유지한다. 의존성 경고가 전혀 없다는 뜻은 아니다.
배포 계약의 native 설치 검사는 로컬 개발 바이너리로 실행했으며 발행 자산 검증이 아니다.

최신 main 반영·발행·이관의 현재 순서와 상태는 [운영 전환 준비](production-readiness.md)를 따른다.
별도 production 콘솔 host·실제 배포 이미지·최종 동일 SHA CI·NoteGate 소비자 계약과
오프라인 이관 리허설의 최종 재실행은 여전히 릴리스 조건이다. 이 검증에서 운영
클러스터·사용자 DB·공급자에 접근하거나 서비스를 배포하지 않았다.

## 검증 지도

경로는 `backend/crates/` 기준이다. 실행 명령은 [기술·운영](../stack/README.md#검증)을 따른다.

| 계약 | 테스트 위치 |
|---|---|
| 역할·표면·자원 권한 | `management-policy/tests`, `management-service/tests/authorization.rs`, `management-service/tests/resources/authorization.rs` |
| 계정·토큰·세션·마지막 Admin | `db/tests/management_accounts.rs`, `db/tests/management_credentials.rs`, `db/tests/management_sessions.rs` |
| 초기화·비밀번호·복구·설정 링크 | `management-service/tests/local_accounts.rs`, `api/src/accounts/tests/password_login`, `db/tests/management_passwords.rs` |
| 재인증 예산·감사 원자성·기록 timeout | `management-service/tests/admission.rs`, `management-service/tests/logging.rs`, `management-service/tests/resource_writes/failures.rs` |
| 관리 로그 보존·경계·배치·잠금·재시도·파일 독립성 | `db/src/retention.rs`, `management-service/tests/retention.rs` |
| multipart 정리 페이지 오류·유계 순회 | `storage-provider/tests/multipart_cleanup.rs` |
| 종료 유예시간·SQLx close 순서·실제 SIGTERM | `api/src/shutdown.rs`, `scripts/e2e-shutdown.py` |
| 자기 이력·타인 필터·cursor | `management-service/tests/history.rs`, `management-service/tests/history_filters.rs`, `api/src/accounts/tests/identity/history.rs` |
| 관리 계정과 파일 요청의 독립성 | `api/src/routes/tests/independence.rs` |
| 브라우저·CSP·권한·비밀·오류 상태 | `frontend/web/tests`, 실제 서버 연결은 `scripts/e2e-console.py` |
| 소비자 호환·새 설치 복원 | `scripts/e2e-notegate.py`, `scripts/e2e-installation.py`, `.github/workflows/ci.yml` |

집중 테스트의 통과를 전체 suite 통과로 표현하지 않는다. 테스트 실패는 오래된
화면 구조 검사, 실제 기능 회귀, 환경 문제를 코드와 실행 결과로 구분한다.
