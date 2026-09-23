# spec 06: 관리 콘솔

- 상태: 구현 계획. `output/` 화면은 샘플 데이터 미리보기다.
- 선행 계약: [관리자 인증](05-admin-auth.md), [CLI](04-cli.md), [등록부](01-registry.md).
- 결정: 기존 관리 API를 공유하고 PostgreSQL을 정본으로 사용한다.

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
| 배포 경로 | `/api/admin/console/`에 정적 파일, `/api/admin/v1`은 기존 서버로 전달 |
| HTTPS | 앞단 TLS 종료, `FILEGATE_CONSOLE_ORIGIN`과 실제 origin 일치 |
| 개발 | 동일 origin HTTPS 프록시 아래 UI·API 연결; Secure 쿠키 계약 유지 |

루트 `/{bucket}/{key}`는 S3 API가 사용한다. 콘솔은 이미 예약된 `api` 경로 아래에
배치해 기존 버킷 이름과 충돌을 피한다. 정적 파일 배포·프록시 배선은 구현 시 검증한다.

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

## 입력과 안전장치

| 영역 | 화면 계약 | 최종 집행 |
|---|---|---|
| S3 등록 | id, endpoint, public_endpoint, region, bucket, path-style, access key, secret, capacity, relay | 서버 필드 검증·접근 검증 |
| fs 등록 | id, root_path, capacity | 서버 경로·쓰기 검증 |
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
| A | 앱 골격·로그인·로그아웃·개요 조회 | 실제 HTTPS 쿠키 로그인, 새로고침 유지, 만료/폐기 401, 로그아웃, readyz·점유 표시 |
| B | 저장소 조회·등록·교체·삭제 | 모든 필드, S3/fs 입력, 실제 409·동시 변경, secret 미보관 |
| C | 클라이언트·Native/S3 키 | CLI 원격 기능 대응, 한 번 표시·폐기, 응답 유실 시 중복 발급 방지 |
| D | 반응형·접근성·배포 | 320/390/768/1024/1440px, light/dark/system, 키보드·초점, 같은 origin 배포 |

각 단계는 단위 테스트·HTTP 통합·실제 브라우저 검증을 갖추고 커밋한다.
미리보기 테스트는 레이아웃 회귀 근거이며 실제 인증·API 연결의 증거와 구분한다.

```text
frontend/web/src/     계획 경로; 아직 생성되지 않음
├── app/              라우팅·초기화
├── api/              HTTP·오류·응답 타입
├── auth/             세션·로그인
├── design/           테마 토큰
├── shared/ui/        입력·버튼·대화상자
├── layout/           내비게이션·반응형 셸
└── features/         overview · storages · clients
```
