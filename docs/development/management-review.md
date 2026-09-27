# Management 정리 준비

기준: `520e172` 코드 점검. 이번 단계는 문서와 준비 계획이며 제품 코드·스키마 변경은 없다.
리소스 구조 마감은 [체크포인트](refactor-checkpoint.md)를 따른다.

## 현재 경계

```text
Console cookie                 CLI / MCP Bearer
    |                              |
    +--> 계정·토큰·세션·이력        |
    |    console_identity          |
    |       -> service::Command    |
    |                              |
    +------> 자원 명령 <------------+
             management-command
                    |
           management-service
           현재 신원 -> policy -> DB 변경 + audit
                    |
             db/management
```

화면의 Resources도 설정 변경은 Management가 소유한다. 파일 본문·업로드 완료·정리 상태는
Object Service/Transfer 경계에 유지한다. 다음 작업은 새로운 서버·crate 추가 없이 시작한다.

## 테이블 관계

```text
Config Root token + generation
    +--> management.master_configuration   설정 세대 fence
    +--> management.root_sessions          Root 콘솔 세션
    +--> management.sessions               기존 setup/recovery 세션

management.accounts                        Admin / Writer / Reader
    +--> management.credentials            이름·hash·만료·폐기
    +--> management.sessions               계정·원본 토큰에 묶인 세션

변경 transaction --------> management.audit_events
요청 처리 후 ------------> management.command_invocations
인증·권한 결과 ----------> management.security_events
로그인 요청 -------------> management.login_budget
```

| 현재 상태 | 코드 근거 | 유지할 이유 |
|---|---|---|
| `users`·`agents` 보조 테이블 제거 완료 | migrations `0012`, `0016` | 같은 신원의 이중 저장 정리 완료 |
| Root는 accounts 행과 별개 | `db/management/root.rs` | 설정 기반 복구 권한과 일반 계정 수명주기 분리 |
| Root·setup 세션 구분 | `management-service/root.rs`, `master.rs` | 일회성 복구 세션이 전체 관리자 세션으로 승격되는 것을 방지 |
| 현재 권한은 잠금 후 판정 | `management-service/lib.rs`, `db/management/transaction.rs` | 토큰 폐기·역할 변경과 명령 실행 순서 보존 |
| resource 설정 변경도 같은 감사 경계 | `db/management/{resource_writes,storage_writes,resource_metadata}.rs` | 성공한 변경과 감사 기록을 함께 확정 |

일반 계정·토큰·세션 관리는 별도 테이블 유지가 수명주기에 맞는다. 과거 감사의
`owner_user_id`, `actor_kind` 값은 현재 계정 모델과 구분되는 역사적 snapshot이다.

## 로그의 최소 책임

| 종류 | 답하는 질문 | 쓰기 계약 | 다음 점검 |
|---|---|---|---|
| Audit | 누가 어떤 설정을 바꿨나? | 변경과 같은 transaction; 기록 실패 시 변경 롤백 | 비밀 제외·정확히 한 번·삭제 후 actor 추적 |
| Command history | 어떤 호출이 성공/실패했나? | best-effort, 쓰기별 250 ms timeout | 조회·폴링이 만드는 증가량, 보존 정책 |
| Security | 인증·권한이 왜 거부됐나? | best-effort, 쓰기별 250 ms timeout | 익명 실패·로그인 예산·민감정보 제외 |

세 종류는 보장 수준이 달라 현재 테이블을 유지한다. 화면 통합 여부와 저장 구조 통합은
별도 결정이다. Client 파일 업로드·다운로드 운영 로그는 이 관리 감사와 별개다.

## 우선순위

| 순서 | 관찰 근거 | 작업 | 완료 기준 |
|---|---|---|---|
| 1. 계약 기준선 | 정책·HTTP·DB에 역할/표면 검증이 이미 존재 | Root/Admin/Writer/Reader × Console/CLI/MCP 테스트와 누락 매핑 | 기존 테스트 실PG 실행, 누락만 추가; 폐기·비활성·마지막 Admin 보호 확인 |
| 2. 이력 비용 | `execute`가 조회에도 invocation 기록, history SQL이 identity transaction 사용 | 반복 조회 시 행 증가·쿼리 계획·락 대기 측정 | baseline 기록 후 보존·인덱스·조회 기록 축소 필요성 결정 |
| 3. 명칭 정리 | UI Accounts/Root, 내부 User/master/access 혼용 | 내부 이름·공개 경로·환경변수·DB 이름 구분 | 내부 변경만 우선; 외부 계약과 과거 actor snapshot 유지 |
| 4. UI 단순화 | Activity의 account/token UUID 입력·3개 stream | 계정 상세에서 이력 탐색·권한별 표시·필터/페이지 검토 | 영어 UI·모바일/태블릿/데스크톱·light/dark·실API 오류 상태 검증 |

1번은 실행 준비 단계다. 2번의 성능 문제·인덱스 필요성은 아직 측정하지 않았으며,
전역 identity lock을 제거하거나 로그 테이블을 합치는 결론으로 사용하지 않는다.

## 테스트 지도

경로는 `backend/crates/` 기준이다. 다음 단계에서 실제 DB 환경을 구성해 실행한다.

| 계약 | 기존 테스트 |
|---|---|
| 역할·표면·자원 권한 | `management-policy/tests`, `management-service/tests/authorization.rs`, `management-service/tests/resources/authorization.rs` |
| 계정·토큰·세션·마지막 Admin | `db/tests/management_accounts.rs`, `db/tests/management_credentials.rs`, `db/tests/management_sessions.rs` |
| Root·세대·복구 | `api/src/console_identity/tests/root.rs`, `management-service/tests/master_fencing.rs`, `management-service/tests/master_recovery.rs` |
| 감사 원자성·기록 실패·시간 제한 | `management-service/tests/logging.rs`, `management-service/tests/resource_writes/failures.rs` |
| 자기 이력·타인 필터·cursor | `management-service/tests/history.rs`, `management-service/tests/history_filters.rs`, `api/src/console_identity/tests/identity/history.rs` |
| 브라우저 보안 | `api/src/console_identity/tests/browser.rs`, `api/src/console_identity/tests/failures.rs` |
| 계정 화면 (repo 기준) | `frontend/web/tests/account-*.spec.ts`, `frontend/web/tests/root-account.spec.ts` |

## 변경 게이트

| 항목 | 기준 |
|---|---|
| 리소스 영향 | Native/S3 API·서비스 키·파일 상태·배치 계약 유지 |
| DB 변경 | 재현된 필요가 있으면 새 migration·업그레이드 테스트로 진행 |
| 호환성 | 기존 운영자 API·관리 토큰·브라우저 세션의 인증 경계 유지 |
| 검증 | 변경 책임의 단위/PG/HTTP 테스트 → 자원 명령 회귀 → 필요한 UI 실연결 |
| 커밋 | 한 책임 변경과 그 회귀 테스트를 한 단계로 기록 |
| 운영 | 배포·실데이터 migration·롤백 리허설은 별도 승인 범위 |
