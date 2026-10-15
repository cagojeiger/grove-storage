# 운영 로그

## 책임

운영 로그는 실패 위치와 처리 경로를 설명한다. 관리 감사는 PostgreSQL에 남는
확정 변경의 기록이다. stdout 유실은 관리 변경을 rollback하지 않으며, 감사 저장
실패는 기존 변경 트랜잭션을 rollback한다. 호출·보안 이력의 best-effort 실패는
`history.write_failed`로 관찰한다.

```text
HTTP request -> request_id
  +-- Management -> operation_id -> PostgreSQL audit/invocation/security
  +-- Object operation -> file_id -> lifecycle transaction
  +-- Receive/send stream -> transfer.finished
Background task -> run_id -> processed/failed/skipped
  |
  +-- safe flat JSON -> bounded stdout queue -> collector -> Loki
```

운영 로그와 후속 Prometheus 지표는 같은 처리 경계와 결과 의미를 사용한다.
지표는 유실 가능한 stdout을 집계한 정본이 아니라 처리 지점에서 직접 관찰한다.
이 변경은 로그 단계이며 메트릭 exporter·수집 설정·알림은 포함하지 않는다.

## 형식과 ID

JSON 한 줄은 `schema_version=1`, `timestamp`, `level`, `target`, `event`, 고정
`message`와 해당 이벤트의 허용 필드를 포함한다. 시각은 주입된 Clock의 UTC이며
소요 시간은 Tokio monotonic Instant로 측정한다. Pretty 출력도 같은 필드 정책을
사용하고 앞에 level을 붙인다. 이전 중첩 `fields`와 `request.end`는 사용하지 않는다.

| 필드 | 의미 |
|---|---|
| `request_id` | HTTP 관찰기가 만든 내부 UUID; 요청과 스트림 진단 연결 |
| `operation_id` | 핸들러가 응답 헤더로 반환한 UUID; Management에서는 DB 이력의 request_id |
| `run_id` | 백그라운드 작업 호출마다 만든 UUID |
| `file_id` | 파일 원장의 내부 UUID |
| `route` | 라우터의 MatchedPath 템플릿; 실제 path·query 아님 |
| `surface` | console·resource_api·mcp·cli·native·s3·system; 이전 관리 경로는 management |
| `duration_ms` | 해당 관찰 경계의 경과 시간; 서로 다른 경계의 값은 합산하지 않음 |

내부 request_id는 외부 `x-request-id`를 덮어쓰지 않는다. 기존 요청 ID의 전파와
응답 헤더 계약은 유지한다. SigV4에 포함된 헤더·URL·본문은 로그 목적으로 변경되지
않는다. operation_id는 핸들러가 UUID를 제공한 응답에만 있으며 요청 전체의 공통 ID는
request_id다. UUID를 Loki stream label로 승격하지 않고 JSON에서 조회한다.

## 이벤트

| 이벤트 | 기록 경계 | 결과·필드 |
|---|---|---|
| `http.response` | 핸들러 응답 생성 | status·surface·method·route·duration_ms·outcome·operation_id |
| `http.interrupted` | 응답 생성 전 future 종료 | interrupted·duration_ms |
| `transfer.finished` | 관찰한 receive/send 스트림 종료 | direction·expected_bytes·observed_bytes·duration_ms·outcome·error_code; send는 file_id 포함 |
| `object.operation_finished` | 예약·파트 기록·완료 확정·논리 삭제 등 명시된 단계 | operation·stage·file_id·outcome |
| `management.operation_finished` | 관리 실행 결과 | operation·operation_id·surface·outcome·error_code·duration_ms |
| `worker.run_finished` | object/retention 작업 호출 종료 | worker·task·run_id·outcome·processed·failed·skipped·duration_ms |
| `history.write_failed` | 호출/보안/파일 접근 이력 저장 실패 또는 시간 초과 | stream·operation_id 또는 file_id·error_code |

복구·보존의 상세 이벤트는 파일 UUID·stream과 고정 error_code를 제공한다.
릴레이 업로드도 `object.operation_finished`의 put/record_upload 또는 upload_part/record_part로
기록한다. record_upload는 lease의 실측 저장이며 파일 활성화 commit과 다르다.
send 로그는 요청 핸들러 종료 뒤에도 request_id와 file_id를 유지한다.
`provider.failed`는 요청 경로의 provider 실패를 나타낸다. provider SDK 응답 본문은
출력하지 않는다. 별도 이벤트가 없는 시작·종료 진단도 같은 안전한 필드 정책을 따른다.

## 콘솔 조회

Activity는 PostgreSQL의 감사·명령 호출·보안 이력을 읽는다. 명령 호출은 결과와
소요 시간을 목록에 표시하며 unknown을 성공이나 실패로 바꾸지 않는다. 상세의
Operation ID는 관리 이력의 request_id이며 운영 로그의 operation_id와 연결된다.
전송·worker 로그는 Activity에 복제하지 않고 Loki에서 조회한다.

## 결과와 한계

HTTP 2xx는 응답 생성의 성공이며 다운로드 완료가 아니다. send의 observed_bytes는
응답 body가 HTTP 런타임에 제공한 바이트이고 수신자의 저장·TCP acknowledgement를
보장하지 않는다. Content-Length만큼 제공되면 success, 이보다 이른 EOF는 incomplete다.
읽기 오류는 error, 종료 전에 body가 버려지면 interrupted다. 원인은 단정하지 않는다.

receive의 observed_bytes는 입력 body에서 읽은 바이트다. success는 임시 스풀 쓰기와
flush 완료이며 외부 S3 저장이나 DB commit의 성공이 아니다. 미달은 incomplete,
선언 크기 초과는 rejected, idle/전체 예산 초과는 timeout, 진행률 미달은 too_slow다. 프리사인드 직결
Client↔외부 S3 바이트는 Grove가 관찰하지 않는다.

worker processed는 확정된 처리·삭제 수, failed는 관찰된 실패 수, skipped는
소유권·재시도 선점 보류 수다. 항목 수와 실패 시도 수는 같은 단위가 아니다.
중단·시간 초과 실행은 전체 집계를 확정하지 않으며 부분 완료를 취소한 것으로
해석하지 않는다. 원격 삭제나 DB commit의 완료 여부는 다음 복구에서 재확인한다.

## 수준과 개인정보

| 수준 | 용도 |
|---|---|
| DEBUG | 정상 HTTP 응답·읽기·URL 발급·완료 스트림·probe·빈 작업; S3 404/412 |
| INFO | 명시된 변경 단계·실제 정리·중단 관찰·느린 정상 요청 |
| WARN | 요청 거부·미달·timeout·재시도 가능한 상세 실패; 미지원 S3 501 |
| ERROR | HTTP 서버 오류·내부 I/O 오류·작업 실패 집계·이력 기록 실패·관리 unknown |

기본 INFO 출력은 정상 DEBUG 이벤트를 제외한다. 전체 요청 수·오류율·지연 분포는
처리 지점의 메트릭으로 측정하며 운영 로그 건수로 대체하지 않는다.

필드 이름과 형식의 allowlist가 DEBUG/TRACE에도 적용된다. payload·raw key·파일명·
presigned URL·auth/header·토큰·계정 이름·IP·자유로운 message/error는 출력하지 않는다.
ID는 UUID, 등록 저장소 ID와 의미 코드는 길이·문자 집합을 제한한다. 의존성 target은
출력 대상이 아니다. 신규 진단은 내부 ID·단계·고정 코드를 추가하고 원문 데이터와
오류 체인을 로그 필드로 사용하지 않는다.

## 출력과 조회

stdout writer는 tracing-appender의 전용 thread와 1,024줄 lossy 큐를 사용한다.
큐 포화는 요청을 기다리게 하지 않는다. `log_dropped_lines_total`은 큐 유실의
프로세스 누적값이며 stdout I/O 오류나 collector/Loki 유실까지 세지 않는다.
guard는 정상 종료 시 큐 drain을 시도한다. SIGKILL·출력 정지·비정상 종료에서는
마지막 기록을 보장하지 않는다. [tracing-appender](https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/index.html)

Loki의 기존 workload selector 뒤에서 JSON 필드를 조회한다. 실제 label 값은
수집기의 설정을 따른다. [LogQL JSON parser](https://grafana.com/docs/loki/latest/query/log_queries/#json)

```logql
{namespace="grove-storage"} | json | event="transfer.finished" | outcome!="success"
{namespace="grove-storage"} | json | event="worker.run_finished" | outcome=~"error|timeout|interrupted"
{namespace="grove-storage"} | json | request_id="<internal-uuid>"
{namespace="grove-storage"} | json | operation_id="<audit-request-uuid>"
```

管理 이력 보존은 [운영 설정](../stack/README.md#관리-로그-보존)을 따른다.
stdout/Loki 보존은 수집 인프라의 정책이며 관리 DB 보존과 독립이다.
수집기는 JSON의 level을 읽는다. error_code나 message에서 수준을 추측하지 않는다.

## 검증

`api/src/logging/tests.rs`는 JSON/Pretty 개인정보 제외, ERROR-only 상관관계,
서명 헤더 유지, HTTP 수준, 송수신 EOF·오류·중단·시간 초과, 가상 시간,
worker 단일 종료, 유계 writer 포화·정상 drain을 검증한다.
전체 API/DB 테스트는 기존 인증·파일 상태·S3 계약의 회귀 범위를 유지한다.
실제 수집기·Loki 조회와 MinIO SDK parity는 별도의 E2E/배포 검증이다.
