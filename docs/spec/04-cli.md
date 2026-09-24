# spec 04: 원격 관리 CLI

- 구현: `gscli` 원격 19개 명령 → [공통 명령 API](09-management-commands.md), User/Agent 관리 토큰.
- 유지: 명령 이름·table/JSON·변경 확인·비밀 파일·수동 update.
- 후속: MCP adapter·연결 profile·운영 인증 이관. 권한·콘솔 전용 경계는 [spec 08](08-management-plane.md).

## 책임 경계

```text
gscli → POST /api/admin/commands/v1 → 현재 User/Agent 권한
                                    → 등록부 변경 + 감사 transaction
                                    → PostgreSQL / backend 접근 검사
filegate status → 로컬 DB·복호 키·저장소 probe
```

| 구성 | 책임 |
|---|---|
| CLI | 인자·설정·파일·확인·HTTP·공개 출력·종료 코드 |
| 공통 명령 계약 | 이름·입출력 DTO·schema·권한 매핑·오류 코드·변경 결과 |
| 서버 실행기 | 현재 role/owner 확인·참조 제약·키 발급·DB 변경·감사 |
| 로컬 진단 | `filegate status`: migration 없이 DB·등록 저장소 접근 확인 |
| 배포 | GitOps·Vault가 프로세스와 비밀 공급; CLI는 DB·서버 복호 키 없이 실행 |

등록부 정본은 PostgreSQL이다. 조회 결과는 비밀 없는 inventory이며 백업 파일이 아니다.
기존 서버의 REST·바이너리·환경변수·데이터 API는 유지한다. 이 CLI는 공통 API와 `gsm_`
토큰이 있는 서버를 대상으로 하며 이전 REST로 자동 fallback하지 않는다. 운영 인증 전환
전의 서버는 기존 CLI 바이너리를 사용한다. 패키지 버전과 command protocol은 별개다.

## CLI·MCP 동등성 계획

| 현재 구현 | 다음 단계 |
|---|---|
| CLI 19개 → 같은 schema·명령명·권한·결과 | MCP tool adapter를 같은 계약에 연결 |
| User/Agent 토큰 → 서버가 현재 권한 판정 | 실제 MCP 전송·오류·비밀 전달 검증 |
| HTTP 호출의 서버 surface는 `resource_api` | 진입점별 surface 계약은 서버에서 결정 |

User·Agent·role·관리 토큰·관리 이력은 콘솔 세션 API 소유다. `credential`·`client-key`는
Client의 서비스 키를 다룬다. CLI의 User-Agent는 신원이나 `surface=cli`의 증거가 아니다.

## 명령 구조

```text
gscli
├── update [--check]
├── status
├── storage
│   ├── list / show ID
│   ├── create ID --from PATH
│   ├── replace ID --from PATH [--yes]
│   └── delete ID [--yes]
├── client
│   ├── list / show ID
│   ├── create ID --storage STORAGE_ID
│   └── delete ID [--yes]
├── credential
│   ├── list --client ID
│   ├── create --client ID --secret-out PATH
│   └── delete --client ID ACCESS_KEY_ID [--yes]
├── client-key
│   ├── list --client ID
│   ├── register --client ID --key-file PATH
│   └── delete --client ID SHA256_HASH [--yes]
└── usage
    ├── storages / clients
    └── history [--days N]
```

`update`는 서버 인증과 독립적인 수동 최신 CLI 설치이고 `--check`는 조회만 한다.
설치·릴리스·업데이트는 [릴리스 계약](../development/releases.md)을 따른다.

| 입력 | 계약 |
|---|---|
| 원격 요청 | POST 한 번, `{protocol:1,command,input}`; 명령별 입력은 spec 09 |
| storage create/replace | `{id,spec}`; `--from PATH`의 spec 최대 1 MiB, `--from -`은 stdin |
| storage spec | JSON object·capacity 필수·알 수 없는 필드 거부; ID는 위치 인자로 공급 |
| replace | 전체 교체; S3 secret을 다시 공급하며 show 결과로 원문을 복구하지 않음 |
| client-key register | raw key 파일 최대 8 KiB·단일 ASCII bearer·말미 LF/CRLF 하나 제거 → SHA-256 |
| resource ID | JSON 문자열로 전송; URL 경로 조합 없이 원값 전달 |
| history | 기본 90일, 1–3650일 |
| 목록 | ID·날짜 기준 정렬, 자동 N+1 조회 없이 현재 목록 표시 |

## 원격 status와 로컬 진단

| 명령 | 확인 범위 | 출력 |
|---|---|---|
| `gscli status` | 공통 `status` 한 번: 서버의 신원·DB·등록부 관찰 | Status DTO·등록 Storage/Client 수 |
| `filegate status` | 기존 로컬 설정·DB·등록 저장소 접근 검사 | 서버 진단 결과 |

CLI의 이전 `/`, `/healthz`, `/readyz`, `/usage`, `/clients` 5회 호출은 공통 명령으로
전환했다. 응답 실패 시 개별 HTTP 상태를 추정한 부분 결과 대신 명령 오류를 반환한다.
유효한 Status DTO에서 필수 상태가 failed/unknown이면 결과를 표시하고 exit 5다.
서버의 health/readiness 값은 현재 명령의 관찰이며 별도 probe URL의 결과가 아니다.

`storage_access=not_checked`는 물리 접근·객체 무결성·워커 진행·전체 replica 건강을
확인하지 않았다는 뜻이다. 등록 0개는 정상이며 미확인 count는 null이다.
바이트는 정수를 보존하고 표에서만 IEC로 표시한다. capacity 0을 무제한으로 해석하지 않는다.

## 연결과 인증

| 설정 | 계약 |
|---|---|
| endpoint | `--endpoint` > `GROVE_ENDPOINT` |
| 관리 토큰 | `--token-file PATH` > `GROVE_TOKEN` > 이전 변수명 `GROVE_OPERATOR_TOKEN` |
| 토큰 종류 | `gsm_` + 소문자 hex 64자, User 또는 Agent; 기존 운영자/master/서비스 키는 로컬에서 거부 |
| 이전 변수명 | 새 관리 토큰을 전달하는 alias만 유지; 이전 인증·REST 선택 기능과 구분 |
| 우선순위 | 우선 설정 값이 비어 있거나 잘못됐으면 오류; 하위 값으로 재시도하지 않음 |
| 파일 | regular file·최대 8 KiB 읽기, 말미 LF/CRLF 한 개 허용 |
| origin | HTTPS, 또는 literal loopback HTTP; userinfo·query·fragment·하위 path 제외 |
| 전송 | TLS 검증·no redirect·no retry; Cookie·OAuth redirect 없이 Bearer만 전송 |
| timeout | HTTP 명령 전체 기본 30초, `--timeout` 1–86400초 |
| 응답 크기 | 최대 8 MiB |
| 로컬 설정 | cwd `.env` 자동 로딩·토큰 원문 인자 없이 명시적 환경변수/파일 사용 |

최초 Admin은 master의 콘솔 설정 API로 만들고, 이후 User/Agent 토큰은 콘솔 세션 API로
발급한다. 화면 연결은 후속이다. API 앞단 proxy는 이 Bearer 경로와 콘솔 OAuth 인증 경계를 분리한다.
연결 profile·OS 키체인은 후속이다. `serve`의 기존 환경변수와 `.env` 로딩은 유지한다.

## 출력과 종료

기본 table, `--output json`은 성공·실패 모두 stdout에 envelope 하나를 출력한다.
help·결과는 stdout, 프롬프트·진단은 stderr다. 파서 오류는 stderr + exit 2다.

```json
{"schema_version":1,"ok":true,"command":"storage.list","endpoint":"https://grove.example.com","data":[],"error":null}
```

| 필드·처리 | 계약 |
|---|---|
| schema_version | CLI 출력 버전 1; HTTP command protocol과 별개 |
| data | 명시적 공개 DTO·정렬된 목록; 비밀·암호화 내부 필드는 제외 |
| error | code·고정 message·http_status·outcome; Provider/HTTP 오류 원문 제외 |
| wire 검증 | protocol·request UUID·command·typed result·대상 ID/Client 일치 검사 |
| 실패 검증 | 구조화 오류의 protocol·request UUID·코드/HTTP 대응 확인 후 outcome 사용 |
| 성공 판정 | HTTP 200만으로 성공을 결정하지 않으며 DELETE도 typed 결과 확인 |

| 종료 코드 | 의미 |
|---|---|
| 0 | 성공 |
| 1 | 로컬 실행·출력 실패 |
| 2 | 인자·설정 오류·취소 |
| 3 | 인증·권한 거부 |
| 4 | 대상 없음 |
| 5 | 연결·TLS·timeout·redirect·서버 장애·응답 오류·status 실패 |
| 6 | 리소스 충돌·설치/업데이트 잠금 충돌 |
| 7 | 입력·protocol·명령 거부 등 |
| 8 | 변경 unknown/applied 실패·비밀 저장 실패·CLI 교체 후 후속 실패 |

## 변경 명령과 비밀

| 경계 | 처리 |
|---|---|
| replace/delete | TTY 확인 또는 `--yes`; 서버 권한·참조 제약은 그대로 적용 |
| 없는 대상 삭제 | 사전 GET 없이 서버의 멱등 성공 결과 사용 |
| 로컬 입력 실패 | HTTP 변경 0회 |
| 변경 요청 뒤 전송/timeout/미검증 응답 | unknown + exit 8; 자동 재시도 없이 목록·show로 대조 |
| 구조화 503/not_applied | 명시적 미적용 결과를 유지하고 exit 5; 5xx만으로 unknown을 추론하지 않음 |
| credential 발급 전 | secret-out을 배타 생성, Unix 0600; 기존 파일·symlink 덮어쓰기 거부 |
| credential 발급 성공 | `{schema_version,client_id,access_key_id,secret_key}`를 파일에 기록·동기화 |
| 확정 미적용 | CLI가 예약한 빈 marker 제거 |
| 응답 불명확 | 빈 marker 유지, 공개 ID가 없으면 null; 명시적 대조·폐기·재발급 |
| 비밀 저장 실패 | applied + exit 8; 알려진 공개 ID와 empty/partial 상태 표시 |
| 비밀 출력 | stdout에는 공개 ID·파일 경로·saved/empty/partial만 표시; `--secret-out -` 제외 |

## 구현·검증 순서

| 단계 | 상태·검증 |
|---|---|
| CLI adapter (4c-1) | 19개 typed 명령·User/Agent 토큰·wire/outcome 검증·비밀/확인/출력 회귀 |
| 실제 CLI E2E | 임시 PG·서버·CLI에서 master 설정·User/Agent 발급·19개 명령·기존 REST 조회 비교·owner 상한·폐기·관리 audit |
| MCP adapter (4c-2) | 후속: 같은 명령 schema·결과·거부와 실제 전송 검증 |
| 운영 이관 | 후속: DB backup·기존 인증 매핑·proxy·소비자 호환·롤백 |

테스트는 `cli/tests`의 설정·조회·입력·변경·비밀·status·wire 파일로 분리한다.
`scripts/e2e-cli.py`는 별도 PostgreSQL·서버를 만들고 종료 시 정리한다.
실제 CLI E2E의 콘솔 쿠키는 HTTP 헤더로 직접 공급하며 브라우저/TLS/proxy E2E와 구분한다.
