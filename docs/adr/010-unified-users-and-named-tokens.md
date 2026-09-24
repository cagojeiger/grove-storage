# ADR 010: User와 용도별 토큰으로 관리 신원을 통합한다

- 상태: 로컬 구현. 운영 전환·배포는 별도 단계.
- 변경: [ADR 009](009-management-identity-and-command-boundary.md)의 User/Agent 분리만 대체한다.
- 유지: 마스터 설정·복구, 콘솔 세션 경계, 자원 명령 공유, Client 데이터 서비스 분리.

## 경계

```text
User (viewer / operator / admin)
├── Token: personal   ── Console session
├── Token: laptop     ── CLI
└── Token: nightly    ── MCP / API
        │
        └── actor_id + credential_id + request_id + surface
             Audit / Calls / Security
```

| 결정 | 계약 |
|---|---|
| 신원 | 관리 계정은 User 하나다. 사람·자동화는 별도 계정 종류로 나누지 않는다 |
| 권한 | 모든 토큰은 소속 User의 현재 역할을 사용한다. 더 낮은 권한의 작업에는 별도 User를 만든다 |
| 토큰 | 이름·공개 ID·만료·폐기 상태를 관리한다. 이름은 설명이며 고유 식별자는 credential_id다 |
| 로그인 | 모든 활성 User 토큰으로 콘솔 세션을 발급받을 수 있다 |
| 진입점 | CLI/MCP/Bearer는 자원 명령을 호출한다. 신원·권한·관리 이력 API는 User 콘솔 세션과 역할·CSRF를 검사한다 |
| 추적 | 같은 User의 토큰도 credential_id로 구분한다. surface는 서버가 결정하며 토큰 이름에서 추정하지 않는다 |
| 폐기 | 토큰 하나를 폐기하면 그 토큰과 연결 세션만 무효화한다. User 삭제는 해당 User의 모든 토큰을 폐기한다 |
| 데이터 서비스 | Clients의 Native/S3 키·Provider secret은 관리 User 토큰과 별도다 |
| 향후 OIDC | 로그인 수단을 기존 User ID에 연결한다. 토큰·감사 ID는 유지한다 |

콘솔 전용 API는 실제 사람이 조작했다는 증명이 아니다. 자동화에 Admin 토큰을 전달하면
그 토큰으로 콘솔 로그인을 수행할 수도 있다. 자동화에 필요한 역할을 가진 User의 토큰을 발급한다.

## 전환

| 데이터 | migration `0012_management_users.sql` |
|---|---|
| 기존 User | ID·토큰·세션 유지 |
| 기존 Agent | 같은 ID·이름·역할의 비활성 User로 전환 |
| 기존 Agent 토큰 | 모두 폐기; 활성화 후 새 토큰 발급으로 명시적 재승인 |
| 소유 관계 | agents 테이블 제거; accounts의 kind는 user로 고정 |
| 과거 이력 | actor_kind·owner_user_id·credential_id snapshot 보존; 과거 owner 이력 조회 유지 |
| Storage·Client·파일 | 변경 없음 |

이전 서버의 쓰기를 중지한 뒤 migration과 새 바이너리를 적용한다. 이전 서버와의 혼합 배포는
지원하지 않는다. 전환 전 DB 백업을 확보하며 롤백은 백업·이전 바이너리를 함께 복원한다.

## 검증

| 범위 | 근거 |
|---|---|
| 정책 | User 역할·콘솔/Bearer 경계, 마지막 Admin 보호 |
| 토큰 기록 | 동일 User의 서로 다른 credential_id, 개별 폐기, 현재 역할 반영 |
| 업그레이드 | 기존 User 세션·토큰과 과거 감사 보존, 전환 Agent 토큰의 재활성화 차단 |
| 화면 | Users 단일 목록·용도별 토큰·일회성 표시, 반응형·권한 변경 |
