# Grove Storage

여러 외부 S3 호환 저장소의 업로드·다운로드를 하나의 서비스 계약으로 연결한다.
클라이언트는 Grove의 Native 또는 지원되는 S3 API를 사용하고, 실제 저장소의
endpoint와 공급자 자격증명은 Grove가 관리한다. 서버 실행 파일은 `grove-storage`,
원격 관리 CLI는 `gscli`다. Native·S3 HTTP 계약은 유지한다. [이관 기록](docs/development/import-review.md)과
[완성도 점검](docs/development/management-review.md)에 검증 범위와 남은 작업을 기록한다.

PostgreSQL에 파일 메타데이터를 기록하고, 외부 S3 호환 저장소의 바이트를
네이티브 API와 지원되는 S3 호환 API로 제공한다. 등록부와 backend는 S3-only다.
DB는 현재 스키마로 새로 초기화하며 기존 FileGate/Grove DB의 인플레이스 업그레이드는 제공하지 않는다.
로컬 임시 스풀은 relay 전송
버퍼이며 등록 가능한 저장소가 아니다. 전체 S3 서비스의 대체를 목표로 하지 않는다.

## 개선 계획

관리 CLI `gscli`가 등록부 조회·변경과 수동 업데이트를 제공한다.
운영 Terraform state 이관과 독립 Agent는 후속 작업이다.
아래 단계는 로드맵이며 현재 제공 기능은 각 spec의 상태 표시를 따른다.

1차는 외부 S3 presigned 전송을 기반으로 관리 CLI와 등록부 Terraform 관리 이관을
진행한다. 2차에 사전 마운트된 파일시스템을 사용하는 Storage Server·Agent를 추가한다.
메타데이터는 PostgreSQL이 중앙 관리하며, 기존 S3·Native API 계약은 유지한다.

## 실행 도구

| 도구 | 실행 위치 | 책임 |
|---|---|---|
| `grove-storage` | 서버·컨테이너 | 서버 실행, 로컬 진단, Account 초기화·복구 |
| `gscli` | 사용자 PC·자동화 환경 | 관리 API를 통한 자원 관리, CLI 자체 업데이트 |

서버 이미지에는 `grove-storage`와 콘솔 정적 파일을 포함한다. `gscli`는 별도 릴리스
자산이며 updater는 `gscli update`에 포함된다. 별도 updater 프로세스는 없다.
서버 설정은 `GROVE_*`, 내부 Rust crate는 `grove-*` 이름을 사용한다.
`FILEGATE_*` 설정은 읽지 않는다. [새 설치·복원 검증](docs/development/fresh-installation.md)을 따른다.

| 문서 | 내용 |
|---|---|
| [문서 목차](docs/README.md) | 현재 구현과 제품 방향 |
| [단계별 제품 결정](docs/adr/007-grove-storage-foundation.md) | 전제·책임·전환 경계 |
| [코드 구조](docs/development/source-layout.md) | 모듈 책임과 테스트 위치 |
| [실행·운영](docs/stack/README.md) | 설정·컨테이너·검증 |
| [S3 연동](docs/guide/s3-onboarding.md) | endpoint·키·버킷·SDK |
| [네이티브 연동](docs/guide/service-integration.md) | 발급·전송·확정 |

## 로컬 실행

```sh
docker compose up -d
cp .env.example .env
export GROVE_DATABASE_URL=postgres://grove:grove@127.0.0.1:55432/grove
cargo run --bin grove-storage -- account init owner Owner
cargo run --bin grove-storage
```

Compose는 PostgreSQL(`55432`), MinIO(`9000/9001`), 개발 버킷을 준비한다.
새 PostgreSQL 볼륨 `grove-pg-data`를 사용하며 이전 개발 DB 볼륨은 변경하지 않는다.
`account init`은 비밀번호를 대화형으로 입력받으며 기본 계정 비밀번호는 없다.
실제 콘솔 연결은 [frontend 실행 절차](frontend/web/README.md#existing-development-api)를
따른다. 서버 이미지는 콘솔 빌드도 포함하며, `GROVE_CONSOLE_ORIGIN`에 지정한
HTTPS 관리 호스트에서 `/api/admin/console/`을 제공한다. [컨테이너 연결](docs/stack/README.md#컨테이너-연결)을 따른다.
콘솔에서 관리 토큰을 발급한 뒤 `gscli`로 storage·client·자격증명을 등록할 수 있다.
구형 운영자 REST API는 제거되어 410을 반환한다. [현행 인증 계약](docs/spec/11-local-management-auth.md)을 따른다.
기존 [Terraform 예제](deploy/local/main.tf)는 이관 전 비교용 구성을 제공한다.
[CLI 등록 절차](docs/guide/registry-management.md)는 Terraform 없는 등록 흐름을 제공한다.

| 확인 | 결과 |
|---|---|
| `GET /` | 이름·버전 |
| `GET /healthz` | 프로세스 생존 |
| `GET /readyz` | DB 준비 상태 |

[릴리스 workflow](.github/workflows/release.yml)는 Grove 저장소의 성공한 main CI를
기준으로 backend 이미지와 Linux/macOS CLI 자산을 발행하도록 구성되어 있다.
서버 이미지는 한 종류이며 Linux amd64/arm64를 지원한다. 취약점·Secret 검사와
출처 검증을 발행 전에 수행하고, 공개 이미지도 매일 재검사한다.
[이미지 보안 정책](docs/development/image-security.md)을 따른다. 전용 Helm 차트는
추가하지 않으며 기존 `quick-deploy` 배포를 유지한다.
이 구성은 실행 환경의 배포 완료를 뜻하지 않는다. 실제 ingress·Secrets·DB 구성과
클러스터 배포 검증은 이미지 빌드와 별도다.

## 관리 CLI

MCP도 같은 자원 명령을 `/api/admin/mcp`에서 제공한다.
[연결·인증·비밀 전달 계약](docs/spec/10-management-mcp.md). 운영 배포는 별도다.

```sh
cargo install --path backend/crates/cli --locked
gscli --endpoint https://api.grove.example.com --token-file /path/to/management-token status
gscli --endpoint https://api.grove.example.com --token-file /path/to/management-token client list --output json
gscli --endpoint https://api.grove.example.com --token-file /path/to/management-token \
  client create notegate --storage primary
```

`GROVE_ENDPOINT`·`GROVE_TOKEN`으로 연결 설정을 공급할 수 있다.
CLI는 Account의 관리 API 토큰으로 공통 자원 명령 API를 호출한다.
계정 생성·비밀번호·관리 토큰·세션 관리는 콘솔 전용이다. 이전 서버에는 이전 CLI를 사용한다.
DB 접속 정보·서버 암호화 키는 CLI에 전달하지 않는다. `grove-storage status`는
서버 로컬 진단으로 유지한다. [명령·출력·후속 계약](docs/spec/04-cli.md).

배포 채널은 GitHub Release의 Linux/macOS 실행 파일이다.
`v0.4.0`은 첫 CLI 자산을 발행한다. 기존 `v0.3.10` 릴리스에는 CLI 자산이 없다.
[버전·설치·업데이트](docs/development/releases.md)를 따른다.
공식 설치본은 `gscli update --check`로 새 버전을 확인하고 `gscli update`로 설치한다.
일반 명령 실행 시 자동 업데이트는 수행하지 않는다.
