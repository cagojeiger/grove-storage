# 기술·운영

## 실행 구조

| 역할 | 구현 |
|---|---|
| 서버 프로세스 | Rust grove-storage 바이너리, axum, tokio |
| 원격 관리 CLI | 독립 gscli 바이너리, clap·reqwest, 관리자 HTTP API |
| 메타데이터 | PostgreSQL + sqlx, 부팅 시 마이그레이션 |
| 바이트 | 외부 S3 I/O는 aws-sdk-s3, 전송 임시 스풀은 tokio filesystem I/O |
| 스트림 | axum body, 임시 스풀, 크기·MD5·SHA256 계측 |
| 비밀 | AES-256-GCM, HKDF, secrecy, 상수시간 비교 |
| 관측 | tracing 구조화 로그 |
| 이미지 | Distroless `cc-debian13:nonroot`, release 바이너리·콘솔·CA 인증서, 비root 사용자 |

의존성 버전은 [Cargo.toml](../../Cargo.toml), 소스 책임은
[소스 구조](../development/source-layout.md), 키 계약은 [등록부](../spec/01-registry.md)가 정본이다.

## 설정

| 설정 | 공급·관리 |
|---|---|
| bind·로그·DB URL·pool 크기·multipart·스풀 예산·CORS | env, 로컬 예시는 [.env.example](../../.env.example) |
| 관리 로그 보존 기간 | env, Audit 365일·Security 90일·Command history 90일; 변경 후 재시작 |
| 마스터 키·key id·이전 키 쌍 | env, [키 회전](../spec/01-registry.md#키와-비밀) |
| 관리자 인증 | 로컬 비밀번호·DB 관리 토큰·콘솔 세션; [spec 11](../spec/11-local-management-auth.md). 이전 REST는 [spec 05](../spec/05-admin-auth.md) |
| storage·client·키·S3 자격증명 | PostgreSQL, 운영자 API |

프로세스는 환경 변수를 읽는다. 배포 도구가 env 또는 Secret을 공급한다.
`scripts/e2e-cli.py`는 Terraform 없이 임시 등록부의 전체 수명주기를 검증한다.
`deploy/local/main.tf`는 운영 이관 완료까지 비교용으로 유지한다.

## 실행 도구

`grove-storage`는 서버 이미지에 포함된 서버·로컬 운영 명령이다. `gscli`는 원격
관리 API에 연결하는 별도 사용자용 바이너리다. 서버 명령은 DB·서버 비밀을 읽고,
원격 CLI는 Account 관리 토큰만 사용한다. Updater는 `gscli` 내부에 포함된다.
서버 설정의 `GROVE_*` 이름은 유지한다.

| 명령 | 현재 동작 |
|---|---|
| `grove-storage`, `grove-storage serve` | 서버 기동·migration·등록 저장소 검증 |
| `grove-storage status` | 로컬 설정으로 DB·저장소 접근 검사, usage·client 수 출력 |
| `grove-storage account init/recover` | 서버 운영자가 최초 Admin 생성·비밀번호 복구; 원격 CLI와 별도 |
| `grove-storage --help` | 명령 도움말 |
| `gscli status` | 원격 HTTP 상태·등록부 요약, 물리 접근은 not_checked |
| `gscli storage/client ...` | 등록부 조회·생성·교체·삭제, metadata 조회·교체 |
| `gscli credential/client-key ...` | 자격증명 발급·키 해시 등록·목록·삭제 |
| `gscli usage ...` | storage·client·일별 사용량 조회 |
| `gscli update [--check]` | 서버 연결 없이 최신 CLI 확인·설치, 공식 설치 기록 검증 |

`grove-storage status`는 HTTP 서버 없이 동작하고 DB URL·마스터 키를 포함한 서버 설정을 읽는다.
DB migration은 수행하지 않으며, 등록된 S3 저장소의 접근을 검사한다.
검사 성공은 exit 0, storage 실패는 exit 1이다. 테스트는 바이트·용량 표현 2개다.
`gscli`은 DB·마스터 키 없이 `GROVE_ENDPOINT`·Account의 `gsm_` 관리 토큰으로 연결한다.
`cargo install --path backend/crates/cli --locked`로 소스에서 설치한다.
명령 계약은 [CLI 스펙](../spec/04-cli.md), 운영 이관은
[등록부 운영](../guide/registry-management.md)을 따른다. 로컬 doctor 개편은 후속 작업이다.

## 컨테이너 연결

```sh
docker build -f deploy/docker/Dockerfile -t grove-storage:dev .
docker run --rm -p 8080:8080 --env-file .env \
  -e GROVE_BIND=0.0.0.0:8080 \
  -e GROVE_DATABASE_URL=postgres://grove:grove@host.docker.internal:55432/grove \
  grove-storage:dev
```

위 실행 예시는 Docker Desktop 기준이다. storage의 내부 endpoint도 컨테이너에서 접근 가능한 주소로 등록한다.

서버 이미지는 Rust 바이너리와 콘솔을 함께 포함한다. Node는 빌드 단계에서만 사용한다.

| 환경 변수 | 동작 |
|---|---|
| `GROVE_CONSOLE_DIST_DIR` | 콘솔 빌드 디렉터리; 이미지 기본값 `/app/web`, 소스 실행은 미설정 |
| `GROVE_CONSOLE_ORIGIN` | 별도 HTTPS 관리 origin. 미설정이면 콘솔 정적 파일과 브라우저 인증이 비활성 |

TLS는 ingress에서 종료하고 원래 `Host`를 보존한다. `X-Forwarded-Host`는 관리 호스트
판정에 사용하지 않는다. 관리 origin이 설정되면 UI 파일 존재 여부와 무관하게 관리 호스트는 콘솔·브라우저
관리 API·OpenAPI 문서·probe만 제공하며, S3·Native·relay 객체 경로는 404다.
다른 호스트에서는 콘솔·브라우저 관리 API가 404이고, 기존 객체·기계용 API 경로를 사용한다.
관리 origin과 객체 origin은 서로 다른 호스트로 구성한다.
관리 origin 설정 시 `GROVE_PUBLIC_URL`이 관리 호스트로 연결되면 DB 연결 전에 부팅을 거부한다.
콘솔 HTML은 응답별 CSP nonce와 `no-store`를 사용한다. 설정한 콘솔 디렉터리의
index·assets가 없거나 nonce 자리표시자가 잘못되면 서버 기동이 실패한다.

실제 이미지 검증은 `python3 -B scripts/e2e-image.py <image>`로 실행한다.
Docker·Node·boto3·`frontend/web`의 npm 의존성·Playwright Chromium이 필요하다.
임시 PostgreSQL·MinIO와 네트워크를 생성해 계정 초기화·HTTPS 로그인·Swagger·호스트 경계·
읽기 전용 실행·종료를 검사한다. 같은 이미지에서 표준 S3 SDK presigned PUT/GET·Range·
잘못된 서명·만료 URL 거부·실제 저장 바이트·삭제도 검증한다. 임시 자원은 정리하고 운영 DB에는 연결하지 않는다.

| 실행 위치 | DB 주소 | MinIO 내부 endpoint |
|---|---|---|
| 호스트 | `127.0.0.1:55432` | `http://127.0.0.1:9000` |
| Docker Desktop 컨테이너 | `host.docker.internal:55432` | `http://host.docker.internal:9000` |
| 같은 Compose 네트워크 | `postgres:5432` | `http://minio:9000` |
| Linux Docker Engine host network | `127.0.0.1:55432` | `http://127.0.0.1:9000` |

Linux의 기존 Compose loopback 공개 주소는
`docker run --rm --network host --env-file .env grove-storage:dev`로 접근한다.
전송 임시 스풀에는 쓰기 가능한 로컬 임시 공간을 제공한다. 영구 객체는 외부 S3에 저장한다.

### 전송 자원 제한

Native relay와 S3 PUT·UploadPart는 같은 프로세스의 스풀 예산을 공유한다.

| 제한 | 동작 |
|---|---|
| 동시 스풀 | 최대 16개; 슬롯이 없으면 대기하지 않고 재시도 오류 |
| 총 예약 용량 | `GROVE_SPOOL_BUDGET_BYTES`, 기본 6GiB; 요청 크기는 MiB 단위로 올림, 예산은 내림 |
| 유휴 시간 | 마지막 비어 있지 않은 청크 이후 30초 |
| 최소 진행량 | 진행 중 전송은 30초 구간마다 평균 64KiB/s 이상 |
| 전체 수신·쓰기 시간 | 마지막 버퍼 flush 포함, 30초 + 선언 크기 / 64KiB/s, 초 단위 올림 |
| 임시 파일 | `$TMPDIR/grove-storage-spool` 또는 OS 임시 디렉터리 아래 전용 영역; Unix 0700/0600, 배타적 생성 |

스풀 슬롯과 용량은 전송 종료·실패·취소 시 반환된다. 정상 종료와 요청 취소는 임시 파일도
제거하며, 프로세스 강제 종료로 남은 파일은 reconciler가 정리한다. Native 단일 relay의
DB claim 슬롯도 대기하지 않는다. S3 용량 부족은 `503 SlowDown`과 `Retry-After`로,
유휴·저속·전체 시간 초과는 `408 RequestTimeout` XML로 반환한다.

예산은 임시 볼륨보다 작게 설정해 잔여 파일과 다른 임시 사용량의 여유를 남긴다.
8GiB 임시 볼륨의 기본 예산은 6GiB다. 이는 프로세스 내 진행 중 전송의 예약 상한이며
파일시스템의 여유 공간 보장은 아니다. 여러 프로세스는 각각의 임시 볼륨을 사용한다.
다운로드 수신 제한과 외부 S3 직결 presigned 전송은 이 스풀 정책에 포함되지 않는다.

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
작업 선택은 [schedule.rs](../../backend/crates/api/src/reconciler/schedule.rs),
각 작업은 [reconciler.rs](../../backend/crates/api/src/reconciler.rs)에 있다.

Native/S3 업로드 완료·정리는 `uploads.recovery_after`, 일반 관찰·reclaim·purge는
`files.recovery_after`와 file ID 순서로 최대 20건을 선택한다. I/O 전에 재시도를
30초 뒤로 기록하고 관찰·물리 정리를 시도당 10초로 제한한다. 실패·시간 초과·
프로세스 종료에도 소유권과 위치는 남는다. 상태가 바뀌면 새 단계는 즉시 시도할 수 있다.
30초는 재시도 가능 시각이며, tick·대기열을 포함한 실제 복구 시간의 상한은 아니다.

Object reconciler 한 회차는 로컬 스풀·DB 풀·잠금 획득을 포함해 20초다. 로컬 스풀
정리는 그 안에서 최대 10초를 사용한다. 종료 신호는 진행 중 회차도 취소한다.
13개 작업은 프로세스 내 cursor로 순환하고, 시간 초과로 중단된 작업 다음부터
다음 회차를 시작한다. Native 완료 관찰과 정리는 별도 작업이다. 재기동 시 작업
순서는 처음부터지만 후보별 재시도 시각은 DB에 남는다. Tokio 제한은 비동기 작업을
취소하며 원격 S3에서 이미 시작된 삭제나 PostgreSQL COMMIT의 취소를 보장하지 않는다.
물리 삭제는 멱등이고, 확정 결과가 불명확하면 다음 관찰·정리에서 장부를 재확인한다.

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
| `GROVE_MANAGEMENT_AUDIT_RETENTION_DAYS` | 365 |
| `GROVE_MANAGEMENT_SECURITY_RETENTION_DAYS` | 90 |
| `GROVE_MANAGEMENT_INVOCATION_RETENTION_DAYS` | 90 |

값은 1–65535의 정수 일수이며 0·음수·잘못된 값은 부팅 시 거부한다. 하루는 24시간이다.
관리 정리는 별도 Tokio task와 retention advisory lock으로 실행한다. 파일 복구가 S3
응답을 기다려도 관리 정리의 잠금을 막지 않는다. 기존 reconciler 주기 값을 공유하며
관리 정리 한 pass는 잠금/DB 풀 획득을 포함해 20초로 제한된다. 관리 정리는 최대
2개 연결의 전용 풀을 사용한다. 프로세스당 DB 연결 상한은 설정된 주 풀 상한 + 2다.
DB 서버와 프로세스 자원은 공유하지만 주 풀 소진으로 관리 정리가 막히지는 않는다.
한 배치의 상한은 1,000건이며 테이블별 2초 예산 안에서 오래된 배치를 반복한다.
관리 이력은 `(created_at,id)`, 파일 접근 이력은 `(at,id)` 순서로 정리한다.
DB 시각에서 보존 기간을 뺀 경계와 같은 시각의 행은 남긴다.
테이블별 별도 transaction과 2초 statement timeout·250ms lock timeout을 사용하고,
pool 대기까지 포함한 반복은 2초로 제한한다. 실패한 테이블은 다음 tick에서 재시도하며
다른 테이블과 파일 작업은 계속 진행한다. 개별 정리 실패는 warn, 실제 삭제 건수와
소요 시간·정리 가능한 만료 행의 가장 오래된 시각은 info, 0건은 debug다.
정리 자체를 관리 호출/감사에 재기록하지 않는다.

같은 독립 작업은 아래 만료 데이터도 정리한다. 계정·등록부·활성 자격증명·파일·lease·
외부 S3 객체는 대상이 아니다. 파일·lease·실물 정리는 object reconciler가 소유한다.

| 데이터 | 정리 기준 |
|---|---|
| 세션 | 만료 또는 폐기 중 이른 시각에서 24시간 경과 |
| 비밀번호 설정 토큰 | 만료 후 24시간 경과 |
| 관리 API 토큰 검증 데이터 | 만료 또는 폐기 후 30일 경과; 참조 세션이 있으면 보류 |
| 파일 접근 이력 | 90일 경과 |
| 사용량 스냅샷 | UTC 날짜 기준 365일 경과 |

토큰 발급·폐기 감사에는 토큰 ID·계정 ID·label·만료 시각이 남고 원문·prefix·검증 hash는
포함하지 않는다. `0005_retention_integrity.sql`은 기존 토큰 감사의 스냅샷을 보강하고
정리 인덱스·파일 이력 ID·사용량 관찰 시각을 추가한다. 기존 관찰 시각은 `NULL`이다.
`0006_upload_ownership.sql`은 Native/S3 완료·정리 소유권을 `uploads`로 통합한다.
`0007_upload_recovery.sql`은 같은 테이블에 재시도 시각과 후보 순서 인덱스를 추가한다.
`0008_file_recovery.sql`은 일반 관찰·회수·purge의 재시도 시각과 순서 인덱스를 추가한다.
파일당 소유권은 하나이며, 프로토콜별 쿼리는 서로의 복구 상태를 변경하지 않는다.
소유권 행은 활성화 또는 물리 정리 성공 후 제거되며 보존 기간으로 삭제하지 않는다.
기간 단축은 다음 실행부터 더 많은 이력을 영구 삭제한다. 기간 연장은 삭제한 기록을
복구하지 않는다. 자동 S3 아카이브는 제공하지 않으므로 필요한 장기 보관은 삭제 전에
별도로 확보해야 한다. DB backup의 보관 정책은 이 작업과 독립이다.

이는 유계 배치 정리이지 정확한 TTL이나 저장 용량 상한 보장이 아니다. 서버 중단,
기존 파일 작업 지연, DB 오류 또는 유입량 초과 시 만료 행이 남을 수 있다. 운영에서는
테이블/인덱스 크기, 만료 행의 가장 오래된 시각, 삭제량·실패, dead tuples와 autovacuum을
확인한다. `DELETE` 직후 디스크 사용량이 곧바로 줄어든다고 가정하지 않는다.

## 검증

```sh
export DATABASE_URL=postgres://grove:grove@127.0.0.1:55432/grove
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

| 검사 | 전제·범위 |
|---|---|
| API·core·infra 테스트 | 순수 단위 테스트와 `DATABASE_URL`이 필요한 API DB 통합 테스트 |
| `db/tests` | PostgreSQL과 `DATABASE_URL`; sqlx가 테스트 DB 생성 |
| `schema_baseline.rs` | 새 DB·재실행·다른 checksum 거절·실패 rollback; PostgreSQL 필수 |
| `scripts/e2e-installation.py` | 새 설치·실제 S3 전송·정확한 DB 백업 복원·미완료 업로드 재개 |
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
