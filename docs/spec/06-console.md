# spec 06: 관리 콘솔

- 상태: A·B단계 로컬 구현·검증, 미릴리스·미배포. `output/`은 기존 샘플 데이터 미리보기다.
- 선행 계약: [관리자 인증](05-admin-auth.md), [CLI](04-cli.md), [등록부](01-registry.md).
- 결정: 기존 관리 API를 공유하고 PostgreSQL을 정본으로 사용한다.
- 브라우저 배포·인증 완료 조건: [보안 경계](07-browser-security.md).

## 구성

```text
Grove Storage
├── 개요       준비 상태 · 등록 수 · 저장소/클라이언트 점유 · 이력
├── 저장소     목록 · 상세 · S3/fs 등록 · 교체 · 삭제
└── 클라이언트 목록 · 상세 · 등록 · 삭제 · Native/S3 키
    공통       로그인 · 로그아웃 · 시스템/라이트/다크
```

| 항목 | 구현 계약 |
|---|---|
| UI | React·TypeScript·Vite, 기존 NoteGate 참고 기록의 semantic token·공통 UI 패턴 |
| 데이터 | 같은 origin의 `/api/admin/v1`; 서버 상태와 폼 입력 상태 분리 |
| 인증 | 토큰으로 세션 발급 후 입력값 제거, 이후 HttpOnly 쿠키 사용 |
| 변경 요청 | `X-FileGate-CSRF: 1`, 서버의 Origin 검사 적용 |
| 브라우저 저장 | 테마 설정만 영속화, 토큰·세션·provider secret은 영속화 대상에서 제외 |
| 배포 경로 | 전용 관리 호스트의 `/api/admin/console/`에 정적 파일, `/api/admin/v1`·`/readyz`만 서버로 전달 |
| HTTPS | 앞단 TLS 종료, `FILEGATE_CONSOLE_ORIGIN`과 실제 origin 일치 |
| 개발 | 동일 origin HTTPS 프록시 아래 UI·API 연결; Secure 쿠키 계약 유지 |

루트 `/{bucket}/{key}`는 별도 데이터 호스트의 S3 API가 사용한다. 콘솔은 이미 예약된
`api` 경로 아래에 배치한다. 관리 호스트는 파일·S3·relay 경로에 404를 반환한다.
정적 파일 배포·프록시 배선은 구현 시 검증한다.

## CLI 대응

아래 API 경로는 `/api/admin/v1` 기준이다. 상태·프로브는 서버의 기존 경로를 사용한다.

| CLI 기능 | 화면 | API / 의미 |
|---|---|---|
| `status` | 개요 | readyz·등록부 조회 조합; 저장소 실물 점검과 구분 |
| `storage list/show` | 저장소 목록·상세 | GET `/storages`, `/storages/{id}` |
| `storage create/replace/delete` | 등록·교체·삭제 | POST `/storages`, PUT/DELETE `/storages/{id}` |
| `client list/show/create/delete` | 클라이언트 | GET/POST `/clients`, GET/DELETE `/clients/{id}` |
| `client-key list/register/delete` | Native 키 | `/clients/{id}/keys`; 입력 raw key의 SHA-256 등록 계약 공유 |
| `credential list/create/delete` | S3 키 | `/clients/{id}/s3-credentials`; secret은 발급 응답에서 한 번 제공 |
| `usage storages/clients/history` | 개요·상세 | `/usage`, `/usage/clients`, `/usage/history?days=N` |
| `update` | CLI 설치 안내 영역 | 사용자 PC의 바이너리 교체는 CLI의 로컬 기능 |

동등성 대상은 원격 관리 작업이다. `filegate admin init/recover`는 운영자 로컬 복구로
유지한다. 원격 관리자 토큰 관리·감사 조회는 현재 API가 없는 후속 기능이다.

## 로그인·토큰 관리 전환

[ADR 008](../adr/008-local-owner-and-agent-credentials.md)의 채택된 방향이며 미구현이다.
권한·DB·전환 순서는 [인증 계획](05-admin-auth.md#로컬-계정토큰-전환-계획)을 따른다.

```text
로그인                  로컬 ID · 비밀번호
설정 / 보안
├── 내 토큰             User 토큰 목록 · 발급 · 폐기
├── 자동화 Agent        생성 · 권한 · 비활성화
│   └── 토큰            Agent별 목록 · 발급 · 폐기
└── 계정                비밀번호 변경 · 로그인 세션 조회/종료
```

토큰 목록은 이름·소유자·만료·최근 사용·상태를 표시한다. 발급 결과에서 원문을 한 번
표시하고 닫을 때 제거한다. User 토큰은 사람의 API 사용, Agent 토큰은 자동화에 사용한다.
`gscli`는 둘 다 전달할 수 있으며 허용 작업은 서버가 계정과 권한으로 판정한다.
클라이언트 화면에서 제공할 Native/S3 키는 파일 서비스 자격증명으로 구분한다.

## 입력과 안전장치

| 영역 | 화면 계약 | 최종 집행 |
|---|---|---|
| S3 등록 | id, endpoint, public_endpoint, region, bucket, path-style, access key, secret, capacity, relay | 서버 필드 검증·접근 검증 |
| fs 등록 | id, root_path, capacity | 서버 경로·쓰기 검증 |
| 등록 용량 | B/GiB/TiB 입력을 정수 bytes로 변환, JSON 정수 정밀도 상한 2^53-1 | 서버 i64 범위의 부분집합 |
| 저장소 교체 | 전체 명세 PUT; secret 재입력, 기존 secret은 조회되지 않음 | location 존재 시 주소 변경 409 |
| 삭제 | 대상 ID 확인, 진행 중 중복 제출 차단, 409 시 이유와 최신 목록 표시 | DB 제약·서버 판단 |
| Native 키 | 평문은 폼 처리 동안만 유지, 등록 후 제거 | 기존 hash 등록 계약 |
| S3 키 발급 | 일회성 secret 표시·다운로드, 닫으면 제거 | 서버 발급·폐기 |
| 401 | 서버 데이터 캐시 제거 후 로그인으로 전환 | 세션 만료·폐기 확인 |
| 429 | Retry-After에 따라 재시도 안내 | 로그인 예산 |
| 변경 응답 유실 | 결과 미확정 표시, 목록 재조회·대조 | 자동 재발급·변경 재전송과 구분 |

목록의 파일 수는 삭제 가능성의 힌트다. 조회 후 동시 쓰기가 발생해도 서버의 409를
그대로 반영한다. 물리 저장소 삭제와 등록부 삭제는 구분한다.

## 구현 순서와 검증

| 단계 | 산출물 | 완료 기준 |
|---|---|---|
| A (구현) | 앱 골격·로그인·로그아웃·개요 조회 | 실제 HTTPS 쿠키 로그인, 새로고침 유지, 만료/폐기 401, 로그아웃, readyz·점유 표시 |
| B (구현) | 저장소 조회·등록·교체·삭제 | 실제 fs/MinIO UI CRUD, 조회 후 참조 추가 409, 주소 교체 409, secret 미보관 |
| 인증 전환 (다음) | 로컬 로그인·User/Agent 토큰 관리 | 인증 spec의 단계 1~4, 실제 HTTPS 로그인·발급·폐기·권한 검증 |
| C | 클라이언트·Native/S3 키 | CLI 원격 기능 대응, 한 번 표시·폐기, 응답 유실 시 중복 발급 방지 |
| D | 반응형·접근성·배포 | 320/390/768/1024/1440px, light/dark/system, 키보드·초점, 같은 origin 배포 |

각 단계는 단위 테스트·HTTP 통합·실제 브라우저 검증을 갖추고 커밋한다.
미리보기 테스트는 레이아웃 회귀 근거이며 실제 인증·API 연결의 증거와 구분한다.

```text
frontend/web/src/     현재 구현
├── app/              라우팅·초기화
├── api/              HTTP·오류·응답 타입
├── auth/             세션·로그인·401 캐시 제거
├── design/           테마·모달·용량 표시
└── features/
    ├── overview/     개요·저장소 점유
    └── storages/     목록·상세·폼·삭제·입력 변환
```

현재 셸은 `app/App.tsx`가 소유한다. 해시 경로로 목록·상세 새로고침과 뒤로 가기를 지원한다.
등록·수정 입력은 폼에 두며 변경 요청의 secret은 query/mutation 캐시에 넣지 않는다.
응답 유실·5xx는 미확정으로 표시하고 재제출을 잠근 뒤 조회로 대조한다.
실행·검증은 [콘솔 README](../../frontend/web/README.md)를 따른다.
개요는 저장소·클라이언트 수와 저장소별 점유를 제공하며, 이력과 클라이언트 상세는 후속이다.
