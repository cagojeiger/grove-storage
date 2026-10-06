# spec 08: 관리 책임·권한·이력

Management는 계정과 등록부 설정을 관리한다. 파일 요청은 Client 자격증명으로
인증하고 관리 Account·세션·역할을 조회하지 않는다.

## 책임

```text
Account -> Management -> 계정·등록부 설정·서비스 키·관리 이력
Client  -> Object Service -> 등록부 읽기 + 파일·위치·lease 갱신
                         -> Transfer -> 외부 S3
```

| 영역 | 소유 책임 | 계약 |
|---|---|---|
| 관리 인증 | 로그인·세션·토큰·초기화·복구 | [로컬 관리 인증](11-local-management-auth.md) |
| 자원 설정 | Storage·Client·서비스 키·metadata | [등록부](01-registry.md) |
| 공통 자원 명령 | Console·CLI·MCP의 입력·출력·실행 | [명령 계약](09-management-commands.md) |
| 관리 권한·이력 | 작업별 권한·조회 범위·기록의 보장 | 이 문서 |
| 파일 수명주기 | 업로드·읽기·삭제·복구 | [Native](00-operations.md) · [S3](03-s3-surface.md) |

Resources 화면은 Management에 속한다. 자원 삭제와 주소 변경은 파일 참조 상태를
확인한다. 관리 계정 폐기와 Client 서비스 키 폐기는 각각의 수명주기를 따른다.
현재 두 영역은 API 프로세스와 데이터베이스를 공유한다.

## 진입점

| 진입점 | 인증 | 작업 범위 |
|---|---|---|
| Console | 비밀번호 로그인 세션·Origin·CSRF | 자원 명령, 역할별 계정 관리·이력, 본인 설정 |
| Management API·CLI·MCP | Account의 관리 API 토큰 | 공통 자원 명령 |
| Native·S3 | Client 서비스 키·서명 URL | 해당 Client의 파일 |
| 서버 로컬 명령 | 서버·DB 운영자 접근 | 최초 Admin 생성·계정 복구 |

관리 API 토큰은 콘솔 로그인 자격증명이 아니다. 토큰 이름은 용도 표시이고,
권한은 소속 Account의 현재 역할에서 결정된다. User-Agent·도구 이름·proxy 사용자
헤더는 신원이나 권한의 근거가 아니다.

## 권한

| 작업 | Reader | Writer | Admin | 진입점 |
|---|---|---|---|---|
| Storage·Client·usage/status 조회 | 허용 | 허용 | 허용 | Console·CLI·MCP·API |
| Storage·Client·metadata 변경 | 거부 | 허용 | 허용 | Console·CLI·MCP·API |
| Client 서비스 키 조회·발급·등록·폐기 | 거부 | 허용 | 허용 | Console·CLI·MCP·API |
| 계정·역할·활성 상태 관리 | 거부 | 거부 | 허용 | Console |
| 다른 계정의 관리 토큰 발급·폐기 | 거부 | 거부 | 허용 | Console |
| 본인 프로필·비밀번호·토큰·세션 | 본인 | 본인 | 본인 | Console |
| 변경 감사·명령 이력 조회 | 본인 | 본인 | 전체 | Console |
| 보안 이벤트 조회 | 거부 | 거부 | 전체 | Console |

본인 범위는 인증된 Account와 그 자격증명의 이력이다. Writer는 Client 서비스 키를
관리하므로 파일 접근 자격증명을 발급할 수 있다. 별도 권한의 자동화는 별도 Account를
사용한다. 역할 변경과 비활성화는 다음 인증 요청에 반영된다.

## 실행 경계

`grove-management-policy`는 검증된 Caller와 서버가 지정한 Surface·Action으로
허용 범위를 결정한다. DB 조회와 원문 자격증명 인증은 서비스·adapter의 책임이다.

```text
HTTP / MCP adapter -> 인증된 요청 -> 현재 계정·권한 확인
                   -> 범위·자원 제약 확인 -> DB 변경 + audit -> commit
```

허용 범위는 설치 전체 또는 본인이다. 서비스는 인증된 ID로 조회 범위를 적용하고
자원 참조 제약을 확인한다. 첫 Admin 생성과 마지막 사용 가능한 Admin의 보호는
DB 잠금으로 직렬화한다. 이 조건은 [인증 계약](11-local-management-auth.md#persistence)에 있다.

변경 감사 저장 실패는 자원 변경도 rollback한다. Commit 결과가 불명확하면
응답은 `unknown`이고 변경 요청은 자동 재시도되지 않는다. 조회·대조 절차는
[등록부 운영](../guide/registry-management.md#변경과-삭제)에 있다.

외부 S3 연결 검사는 DB transaction과 함께 rollback되지 않는다. Storage 생성·교체는
접근 검사 뒤 현재 권한과 설정을 재확인하고 등록 변경을 확정한다.

## 관리 이력

| 저장소 | 기록 대상 | 보장 |
|---|---|---|
| `management.audit_events` | 확정된 계정·설정·자격증명 변경 | 변경과 같은 transaction; 기록 실패는 rollback |
| `management.command_invocations` | 인증 후 관리 작업의 조회·성공·실패 | best-effort; 기록 실패는 작업 결과를 바꾸지 않음 |
| `management.security_events` | 인증·권한 검사 결과 | 인증 전에는 actor가 불명일 수 있음; 저장 실패는 운영 로그에 기록 |

```text
request_id=R: MCP token -> storage.replace
  command_invocations: Account / credential / surface / outcome / duration
  audit_events:        Account / Storage ID / 변경 정보 / request_id=R
```

서버는 request_id와 surface를 지정한다. 인증 후 권한 거부는 호출·보안 이력에
기록되고 변경 감사는 생성되지 않는다. 요청 형식 오류와 서버 도달 전 실패는
관리 명령 실행 이력의 범위 밖이다.

로그는 operation별 허용 필드와 안정된 오류 코드를 사용한다. 비밀번호·토큰·S3 secret·
Cookie·암호문·presigned URL·metadata 원문은 관리 이력에 저장되지 않는다.
개별 이력 수정·삭제 API는 없고 기간별 정리가 기록을 제거한다. DB 관리자에 대한
변조 방지는 이 저장 계약에 포함되지 않는다.

| 기록 영역 | 관찰 범위 |
|---|---|
| Activity | 계정·등록부·서비스 키의 관리 변경과 호출 |
| `lease_history` | 파일 접근 lease의 발급·상태·시간·크기 |
| Runtime tracing | Native·S3 요청과 작업 결과 |
| Provider 로그 | 외부 S3 직결 전송의 사용·완료 |

Lease 발급은 전송 완료의 증거가 아니다. Grove는 외부 S3로 직접 내려받는 요청의
완료나 서명 URL의 반복 사용을 직접 관찰하지 않는다. Runtime tracing은 별도 로그
수집기의 보관 정책을 따른다.

관리 로그의 보존 기간·배치·실패 처리·설정은
[관리 로그 운영](../stack/README.md#관리-로그-보존)에 있다.

## 구현 근거

| 책임 | 위치 |
|---|---|
| 순수 권한 | `backend/crates/management-policy` |
| 현재 신원·작업 실행 | `backend/crates/management-service` |
| 저장·조회 범위·감사 transaction | `backend/crates/db/src/management` |
| Console·Bearer·MCP 요청 | `backend/crates/api/src`의 각 adapter |

테스트 위치와 의존성은 [소스 구조](../development/source-layout.md#검증-위치)에 있다.
이전 신원 모델의 결정은 [ADR 목차](../adr/README.md)에 남아 있다.
