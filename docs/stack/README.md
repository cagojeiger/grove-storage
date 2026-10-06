# 기술·운영

## 실행 구조

| 역할 | 구현 |
|---|---|
| 서버 프로세스 | Rust filegate 바이너리, axum, tokio |
| 원격 관리 CLI | 독립 gscli 바이너리, clap·reqwest, 관리자 HTTP API |
| 메타데이터 | PostgreSQL + sqlx, 부팅 시 마이그레이션 |
| 바이트 | 외부 S3 I/O는 aws-sdk-s3, 전송 임시 스풀은 tokio filesystem I/O |
| 스트림 | axum body, 임시 스풀, 크기·MD5·SHA256 계측 |
| 비밀 | AES-256-GCM, HKDF, secrecy, 상수시간 비교 |
| 관측 | tracing 구조화 로그 |
| 이미지 | `debian:bookworm-slim`, release 바이너리·CA 인증서, 비root 사용자 |

의존성 버전은 [Cargo.toml](../../Cargo.toml), 소스 책임은
[소스 구조](../development/source-layout.md), 키 계약은 [등록부](../spec/01-registry.md)가 정본이다.

## 설정

| 설정 | 공급·관리 |
|---|---|
| bind·로그·DB URL·pool 크기·multipart·CORS | env, 로컬 예시는 [.env.example](../../.env.example) |
| 관리 로그 보존 기간 | env, Audit 365일·Security 90일·Command history 30일; 변경 후 재시작 |
| 마스터 키·key id·이전 키 쌍 | env, [키 회전](../spec/01-registry.md#키와-비밀) |
| 관리자 인증 | 로컬 비밀번호·DB 관리 토큰·콘솔 세션; [spec 11](../spec/11-local-management-auth.md). 이전 REST는 [spec 05](../spec/05-admin-auth.md) |
| storage·client·키·S3 자격증명 | PostgreSQL, 운영자 API |

프로세스는 환경 변수를 읽는다. 배포 도구가 env 또는 Secret을 공급한다.
`scripts/e2e-cli.py`는 Terraform 없이 임시 등록부의 전체 수명주기를 검증한다.
`deploy/local/main.tf`는 운영 이관 완료까지 비교용으로 유지한다.

## 현재 CLI

| 명령 | 현재 동작 |
|---|---|
| `filegate`, `filegate serve` | 서버 기동·migration·등록 저장소 검증 |
| `filegate status` | 로컬 설정으로 DB·저장소 접근 검사, usage·client 수 출력 |
| `filegate account init/recover` | 서버 운영자가 최초 Admin 생성·비밀번호 복구; 원격 CLI와 별도 |
| `filegate --help` | 명령 도움말 |
| `gscli status` | 원격 HTTP 상태·등록부 요약, 물리 접근은 not_checked |
| `gscli storage/client ...` | 등록부 조회·생성·교체·삭제, metadata 조회·교체 |
| `gscli credential/client-key ...` | 자격증명 발급·키 해시 등록·목록·삭제 |
| `gscli usage ...` | storage·client·일별 사용량 조회 |
| `gscli update [--check]` | 서버 연결 없이 최신 CLI 확인·설치, 공식 설치 기록 검증 |

`filegate status`는 HTTP 서버 없이 동작하고 DB URL·마스터 키를 포함한 서버 설정을 읽는다.
DB migration은 수행하지 않으며, 등록된 S3 저장소의 접근을 검사한다.
검사 성공은 exit 0, storage 실패는 exit 1이다. 테스트는 바이트·용량 표현 2개다.
`gscli`은 DB·마스터 키 없이 `GROVE_ENDPOINT`·Account의 `gsm_` 관리 토큰으로 연결한다.
`cargo install --path backend/crates/cli --locked`로 소스에서 설치한다.
명령 계약은 [CLI 스펙](../spec/04-cli.md), 운영 이관은
[등록부 운영](../guide/registry-management.md)을 따른다. 로컬 doctor 개편은 후속 작업이다.

## 컨테이너 연결

```sh
docker build -f deploy/docker/Dockerfile -t filegate:dev .
docker run --rm -p 8080:8080 --env-file .env \
  -e FILEGATE_BIND=0.0.0.0:8080 \
  -e FILEGATE_DATABASE_URL=postgres://filegate:filegate@host.docker.internal:55432/filegate \
  filegate:dev
```

위 실행 예시는 Docker Desktop 기준이다. storage의 내부 endpoint도 컨테이너에서 접근 가능한 주소로 등록한다.

| 실행 위치 | DB 주소 | MinIO 내부 endpoint |
|---|---|---|
| 호스트 | `127.0.0.1:55432` | `http://127.0.0.1:9000` |
| Docker Desktop 컨테이너 | `host.docker.internal:55432` | `http://host.docker.internal:9000` |
| 같은 Compose 네트워크 | `postgres:5432` | `http://minio:9000` |
| Linux Docker Engine host network | `127.0.0.1:55432` | `http://127.0.0.1:9000` |

Linux의 기존 Compose loopback 공개 주소는
`docker run --rm --network host --env-file .env filegate:dev`로 접근한다.
전송 임시 스풀에는 쓰기 가능한 로컬 임시 공간을 제공한다. 영구 객체는 외부 S3에 저장한다.

## 워커

```mermaid
flowchart LR
    Boot["설정 / DB 연결 / migration / storage 검증"] --> Run["HTTP + object reconciler + management retention"]
    Run --> Tick["tick"]
    Tick --> Lock["PostgreSQL advisory transaction lock"]
    Lock --> Jobs["관찰 / 복구 / 정리 / 사용량 스냅샷"]
```

| 조건 | 동작 |
|---|---|
| 여러 API 프로세스 | DB 트랜잭션으로 전이 직렬화 |
| 워커 tick | 파일 복구와 관리 로그 정리를 별도 작업·advisory lock으로 실행 |
| 연결·프로세스 종료 | 트랜잭션 락 자동 해제 |
| 로컬 임시 스풀 | 각 프로세스가 자기 임시 영역 정리 |
| 종료 신호 | HTTP·워커 드레인과 DB 풀 닫기를 총 20초 이내 대기 |

후보 스캔은 작업별 배치로 처리하며, 일별 스냅샷은 등록부 사용량을 집계한다.
작업 순서는 [reconciler.rs](../../backend/crates/api/src/reconciler.rs)에 있다.

종료 유예시간을 초과하면 모든 백그라운드 작업을 취소하고 프로세스가 오류 종료한다. 컨테이너의
종료 유예시간은 이 20초보다 길게 설정한다. 진행 중 릴레이 연결은 중단될 수 있으며,
물리 정리가 확정되지 않은 작업의 DB 상태는 다음 기동에서 재시도한다.
이미 발급된 presigned URL의 외부 S3 쓰기는 프로세스 종료로 취소되지 않는다.
업로드 ID 유실 복구의 multipart 조회는 같은 marker 쌍의 반복·순환, 누락된 key
marker 또는 1,000페이지 초과 시 실패한다. 정리 장부를 확정하지 않고 다음 tick에서
재시도하여 다른 파일 작업이 계속 실행되게 한다.

### 관리 로그 보존

| 환경 변수 | 기본값 |
|---|---:|
| `FILEGATE_MANAGEMENT_AUDIT_RETENTION_DAYS` | 365 |
| `FILEGATE_MANAGEMENT_SECURITY_RETENTION_DAYS` | 90 |
| `FILEGATE_MANAGEMENT_INVOCATION_RETENTION_DAYS` | 30 |

값은 1–65535의 정수 일수이며 0·음수·잘못된 값은 부팅 시 거부한다. 하루는 24시간이다.
관리 정리는 별도 Tokio task와 retention advisory lock으로 실행한다. 파일 복구가 S3
응답을 기다려도 관리 정리의 잠금을 막지 않는다. 기존 reconciler 주기 값을 공유하며
관리 정리 한 pass는 잠금/DB 풀 획득을 포함해 20초로 제한된다. 관리 정리는 최대
2개 연결의 전용 풀을 사용한다. 프로세스당 DB 연결 상한은 설정된 주 풀 상한 + 2다.
DB 서버와 프로세스 자원은 공유하지만 주 풀 소진으로 관리 정리가 막히지는 않는다.
각 테이블의 DB 시각에서 보존 기간을 뺀 기준보다 오래된 `created_at`만,
`created_at,id` 순서로 tick당 최대 1,000건 삭제한다. 경계 시각과 최신 행은 보존한다.
테이블별 별도 transaction과 2초 statement timeout·250ms lock timeout을 사용하고,
pool 대기까지 포함한 호출은 5초로 제한한다. 실패한 테이블은 다음 tick에서 재시도하며
다른 테이블과 파일 작업은 계속 진행한다. 개별 정리 실패는 warn, 실제 삭제 건수와
소요 시간은 info, 0건은 debug다. 정리 자체를 관리 호출/감사에 재기록하지 않는다.

계정·자격증명·세션·등록부·파일·lease·사용량 및 외부 S3 객체는 삭제 대상이 아니다.
기존 `(created_at,id)` 인덱스에 맞춘 조회 순서를 사용하며 별도 migration은 없다.
기간 단축은 다음 실행부터 더 많은 이력을 영구 삭제한다. 기간 연장은 삭제한 기록을
복구하지 않는다. 자동 S3 아카이브는 제공하지 않으므로 필요한 장기 보관은 삭제 전에
별도로 확보해야 한다. DB backup의 보관 정책은 이 작업과 독립이다.

이는 유계 배치 정리이지 정확한 TTL이나 저장 용량 상한 보장이 아니다. 서버 중단,
기존 파일 작업 지연, DB 오류 또는 유입량 초과 시 만료 행이 남을 수 있다. 운영에서는
테이블/인덱스 크기, 만료 행의 가장 오래된 시각, 삭제량·실패, dead tuples와 autovacuum을
확인한다. `DELETE` 직후 디스크 사용량이 곧바로 줄어든다고 가정하지 않는다.

## 검증

```sh
export DATABASE_URL=postgres://filegate:filegate@127.0.0.1:55432/filegate
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

| 검사 | 전제·범위 |
|---|---|
| API·core·infra 단위 테스트 | DB 연결 없는 테스트 |
| `db/tests` | PostgreSQL과 `DATABASE_URL`; sqlx가 테스트 DB 생성 |
| `migrations.rs` | URL이 있으면 migration 검증, 없으면 조기 반환 |
| `scripts/e2e-*.sh` | 로컬 전용 DB·MinIO·서버·스크립트별 등록 전제 |
| `scripts/e2e-registry.sh` | 시작·종료 시 등록부 초기화, 전용 개발 DB에서 실행 |
| `scripts/e2e-cli.py` | 임시 PostgreSQL·실제 서버에서 CLI 등록·조회·삭제 수명주기 |
| `scripts/e2e-shutdown.py` | 임시 PostgreSQL·실제 서버의 SIGTERM 정상 종료·HTTP/워커/DB 드레인 로그 검증 |
| `scripts/s3-capture.py` | S3 endpoint·자격증명·bucket으로 실제 객체·multipart 검증 |

## 로그

| 레벨 | 대상 |
|---|---|
| info | 부팅·종료·실제 요청 |
| debug | 반복 tick·락 획득 실패 |
| warn | 재시도 가능한 이상 |
| error | 요청·워커·준비 상태 실패 |

성공한 health/readiness 요청은 로그에서 제외하고 실패를 기록한다.
