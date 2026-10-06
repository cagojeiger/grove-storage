# 문서

| 읽을 내용 | 정본 |
|---|---|
| 단계별 제품 방향과 구현 경계 | [ADR 007](adr/007-grove-storage-foundation.md) |
| 결정과 근거 | [ADR 목차](adr/README.md) |
| 현재 네이티브 파일 API | [파일](spec/00-operations.md) · [multipart](spec/02-multipart.md) |
| 현재 S3 API | [S3 계약](spec/03-s3-surface.md) |
| 등록·인증·키 회전 | [등록부](spec/01-registry.md) |
| 현행 관리자 로그인·초기화·복구 | [로컬 관리 인증](spec/11-local-management-auth.md) |
| 관리 책임·권한·이력 | [관리 경계](spec/08-management-plane.md) |
| 이전 운영자 REST 인증 | [spec 05](spec/05-admin-auth.md) |
| 관리 MCP 연결·CLI 대응·비밀 전달 | [spec 10](spec/10-management-mcp.md) |
| CLI/MCP 공통 명령·입출력·오류·권한 계약 | [명령 계약](spec/09-management-commands.md) |
| 관리 콘솔의 화면·API 대응·검증 경계 | [콘솔 계약](spec/06-console.md) |
| 브라우저 origin·CSP·proxy·토큰/세션 보안 | [브라우저 보안](spec/07-browser-security.md) |
| 로컬 진단 / 원격 관리 CLI | [status](stack/README.md#현재-cli) · [CLI 구현·후속 계약](spec/04-cli.md) |
| 버전 / CLI 설치·배포 | [릴리스 계약](development/releases.md) |
| 서비스 연결 | [네이티브](guide/service-integration.md) · [S3](guide/s3-onboarding.md) |
| 등록부 운영·Terraform 이관 | [CLI 등록 절차](guide/registry-management.md) |
| 코드 책임·테스트 위치 | [소스 구조](development/source-layout.md) |
| 시간 의존성·만료 경계·가상 시간 테스트 | [시간 테스트](development/time-testing.md) |
| 리소스 구조 마감·검증 경계 | [리소스 체크포인트](development/refactor-checkpoint.md) · [S3/NoteGate 검증](development/s3-compatibility-review.md) |
| Management 완성도·운영 준비 우선순위·테스트 지도 | [완성도 점검](development/management-review.md) |
| main 반영·릴리스·FileGate 운영 이관 게이트 | [운영 전환 준비](development/production-readiness.md) |
| 실행·설정·검증 | [기술·운영](stack/README.md) |
| 외부 저장소 조사 기록 | [벤더 노트](vendors/README.md) |
| 과거 코드 이관·단계별 검증 기록 | [이관 분석](development/import-review.md) |
| 과거 로컬 인증 단계별 검증 기록 | [인증 검증 기록](development/local-auth-verification.md) |

## 현재와 방향

현재 구현과 다음 작업을 구분한다. 로컬 검증과 운영 전환 상태는 별도다.

| 축 | 현재 구현 | 다음 작업 | 2차 확장 계획 |
|---|---|---|---|
| 메타데이터 | PostgreSQL 정본 | 중앙 관리 유지 | Node·Root·작업 상태 추가 |
| 바이트 | 외부 S3 presigned·relay, 로컬은 전송 임시 스풀 | 운영 endpoint 검증 | Agent가 mounted filesystem I/O 수행 |
| 클라이언트 | 네이티브 + S3 중계 API | 현행 계약 유지, 소비자 전환은 별도 결정 | 독자 스토리지의 S3 호환 계약 구체화 |
| 배치 | client에 storage 하나 고정 | 현행 배치 유지 | 조인·배치·이동 모델 설계 |
| 복구 | 업로드 완료·삭제·만료 복구 | 현행 복구 유지 | Node 장애·작업 복구 설계 |
| 관리 | 운영자 API, gscli 조회·변경, Terraform 비교 예제 | 등록부 Terraform 운영 이관 | Storage Server·Agent 조인 |
| 관리 인증 | 로컬 비밀번호·Accounts·관리 토큰·브라우저 세션; 이전 REST는 명시적 호환 모드 | 운영 인증 이관·proxy 검증 | OIDC·노드 조인 자격증명은 별도 계약 |
| 관리 명령·감사 | CLI/MCP/콘솔 공통 자원 실행기, 변경 audit·호출·보안 로그·Activity UI | 로그 보존·조회 비용 점검, 운영 전환 | 데이터 경로의 로그와 별도 |
| 실행 이름 | 서버 `filegate`·`FILEGATE_*`, CLI `gscli`·`GROVE_*` | 기존 서버 계약 유지 | 서버 이름 변경은 별도 릴리스 |

Grove Storage는 후속 제품명이다. 현재 등록부와 backend는 S3-only다.
Migration `0017`은 FS 행이 있으면 중단하며, 독립 filesystem Node는 2차 설계 대상이다.
운영 이관 완료 여부는 로컬 코드·검증과 구분하고 현재 계약은 spec을 따른다.

## 문서 책임

| 문서 | 소유 내용 |
|---|---|
| spec | 현재 입력·출력·상태 전이·실패 계약 |
| guide | 설치·연동·운영 절차 |
| ADR | 결정 당시의 이유와 대체 상태 |
| development | 코드 구조·테스트 위치·날짜와 revision이 있는 검증 기록 |
| stack | 실행 구성·환경 설정·워커 운영 |

같은 계약은 한 문서에서 설명하고 다른 문서는 링크로 연결한다. 과거 검증 기록은
당시 revision의 결과이며 현재 릴리스·배포 상태와 구분된다.

## 문장과 표현

문장은 대상·조건·동작·결과를 설명한다. 지시형 문장이나 예외를 덧붙이는 설명보다
현재 동작과 적용 범위를 직접 적는다. 배경은 결정 이유를 이해하는 데 필요한 만큼 둔다.

| 내용 | 표현 예 |
|---|---|
| 실패 결과 | 감사 저장 실패는 변경도 rollback한다. |
| 적용 조건 | Legacy REST는 호환 모드에서 제공한다. 기본 설정의 응답은 410이다. |
| 운영 안전 | 업그레이드 구간에는 모든 이전 DB writer가 중지된 상태다. |
| 복원 경계 | 롤백은 이전 바이너리와 대응하는 DB 백업으로 복원한다. |

| 표현 | 용도 |
|---|---|
| Mermaid | 책임 관계, 요청 흐름, 상태 전이 |
| ASCII 트리 | 디렉터리와 물리 파일 배치 |
| Markdown 표 | 입력·출력·실패, 책임, 지원 상태 |
| 짧은 문장 | 결정 이유, 원자성·보존 조건 |

주석은 락 순서·인코딩·복구 재료처럼 코드만으로 드러나지 않는 조건을 설명한다.
