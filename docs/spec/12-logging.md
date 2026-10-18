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
  +-- safe structured JSON -> bounded stdout queue -> collector -> Loki
```

운영 로그와 후속 Prometheus 지표는 같은 처리 경계와 결과 의미를 사용한다.
지표는 유실 가능한 stdout을 집계한 정본이 아니라 처리 지점에서 직접 관찰한다.
직접 계측과 exporter는 [메트릭 계약](13-metrics.md)을 따른다. 수집 설정·알림·
Grafana 대시보드는 별도 배포 단계다.

## 형식과 ID

JSON 한 줄은 OpenTelemetry LogRecord 데이터 모델에 매핑되는 stdout 표현이다.
OTLP JSON 전송 형식은 아니며 OTel SDK·exporter는 아직 연결하지 않는다.
실제 OTLP 전송은 공식 SDK의 직렬화·전송을 사용한다.

| 출력 필드 | OTel 대응 | 값 |
|---|---|---|
| timestamp | Timestamp | 주입된 Clock의 UTC RFC3339, 밀리초 정밀도 |
| severity_text·severity_number | SeverityText·SeverityNumber | TRACE=1·DEBUG=5·INFO=9·WARN=13·ERROR=17 |
| body | Body | 원문 message 대신 고정 설명 |
| event_name | EventName | `grove.` 접두어와 아래 이벤트 코드 |
| resource | Resource | `service.name=grove-storage`·빌드의 `service.version` |
| instrumentation_scope | InstrumentationScope | `name=grove-storage.logs`·빌드 버전 |
| attributes | Attributes | 표준 HTTP 속성과 허용된 `grove.*` 작업 속성 |

발생 시각과 수집 시각은 구분한다. 수집기 관측 시각인 ObservedTimestamp와 실제
추적 문맥인 TraceId·SpanId·TraceFlags는 현재 출력하지 않는다. 작업 UUID를
TraceId로 바꾸거나 없는 추적 문맥을 생성하지 않는다. 자체 schema_version은
출력하지 않으며 이 문서의 stdout 표현과 OTLP 인코딩을 혼용하지 않는다.
Pretty는 같은 JSON 앞에 수준을 붙인다. 소요 시간은 Tokio monotonic Instant다.

[OTel 로그 데이터 모델](https://opentelemetry.io/docs/specs/otel/logs/data-model/),
[HTTP 의미 규약](https://opentelemetry.io/docs/specs/semconv/http/http-spans/)

| 기존 필드 | attributes의 키 |
|---|---|
| method·status·route | http.request.method·http.response.status_code·http.route |
| request_id·operation_id·run_id·file_id | grove.request_id·grove.operation_id·grove.run_id·grove.file_id |
| operation·stage·outcome·duration_ms | grove.operation·grove.stage·grove.outcome·grove.duration_ms |
| error_code | grove.error_code; 의미가 다른 error.type으로 일괄 치환하지 않음 |
| url_scheme·error_type | url.scheme·error.type; 관찰한 HTTP scheme과 5xx 상태 코드 |
| 그 외 허용된 필드 | grove 접두어와 기존 이름 |
| Rust target | grove.code_target; 계측 범위와 별개 |

아래 표의 필드는 계측 지점에서 사용하는 이름이다. 출력은 위 매핑을 적용하고
숫자·boolean 자료형을 유지한다. 예를 들어 `http.response`의 출력 이름은
`grove.http.response`다. HTTP route는 실제 path가 아닌 라우트 템플릿이다.

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
request_id다. UUID를 Loki stream label로 승격하지 않고 attributes에서 조회한다.

## 이벤트

| 이벤트 | 기록 경계 | 결과·필드 |
|---|---|---|
| `http.response` | 핸들러 응답 생성 | status·surface·method·route·duration_ms·outcome·operation_id |
| `http.request_finished` | 로컬 응답 body 종료 또는 요청 중단 | request_id·status(응답이 있으면)·duration_ms·outcome·error_code; S3는 operation·stage·s3_error_code |
| `http.interrupted` | 응답 생성 전 future 종료 | interrupted·duration_ms |
| `transfer.finished` | 관찰한 receive/send 스트림 종료 | direction·expected_bytes·observed_bytes·duration_ms·outcome·error_code; send는 file_id 포함 |
| `object.operation_finished` | 예약·파트 기록·완료 확정·논리 삭제 등 명시된 단계 | operation·stage·file_id·outcome |
| `management.operation_finished` | 관리 실행 결과 | operation·operation_id·surface·outcome·error_code·duration_ms |
| `worker.run_finished` | object/retention 작업 호출 종료 | worker·task·run_id·outcome·processed·failed·skipped·duration_ms |
| `history.write_failed` | 호출/보안/파일 접근 이력 저장 실패 또는 시간 초과 | stream·operation_id 또는 file_id·error_code |
| `database.snapshot_failed` | DB 메트릭 수집 실패 또는 회차 제한 | run_id·worker·task·stage·stream·reason·error_code |

복구·보존의 상세 이벤트는 파일 UUID·stream과 고정 error_code를 제공한다.
릴레이 업로드도 `object.operation_finished`의 put/record_upload 또는 upload_part/record_part로
기록한다. record_upload는 lease의 실측 저장이며 파일 활성화 commit과 다르다.
send 로그는 요청 핸들러 종료 뒤에도 request_id와 file_id를 유지한다.
`provider.failed`는 요청 경로의 provider 실패를 나타낸다. provider SDK 응답 본문은
출력하지 않는다. 별도 이벤트가 없는 시작·종료 진단도 같은 안전한 필드 정책을 따른다.

`http.request_finished`는 기존 HTTP 메트릭의 본문 종료 관측을 재사용한다.
HEAD·본문 없는 상태·빈 응답은 해당 종료 경계에서 기록한다. 그 외는 완료·길이 불일치·
body 오류·drop 중 먼저 관찰한 종료를 한 번 기록한다. 정상은 INFO, 요청 거부는 WARN,
서버·body 오류는 ERROR, 중단은 INFO다. probe·루트 서비스 정보·콘솔 정적 파일은 제외한다.
기본 INFO에서도 정상 서비스 요청을 기록하지만 수집 무손실을 보장하지 않는다.

S3 작업명은 인증·버킷 검사 뒤 확정된 Put/Get/Head/Delete와 multipart 4종의 고정 코드다.
그 이전 거부나 분류 실패는 unknown이다. stage는 마지막으로 관찰한 dispatch 경계인
authentication·authorization·routing·validation·execution이며 내부 DB 실패 단계를
추정하지 않는다. s3_error_code는 응답 내부 메타데이터의 고정 XML 코드다. 본문을 다시
읽거나 오류 message를 기록하지 않으며 wire 응답과 SigV4 입력은 변경하지 않는다.
body 오류가 나면 error_code는 해당 종료 사유이고 s3_error_code는 원래 응답 코드를 유지한다.

수집 후 요청 종료 행의 중복 여부는 event_name과 내부 request_id로 구분한다.
클라이언트 재시도는 새 request_id의 별도 요청이다. 요청량은 이 이벤트만 집계하고
http.response·transfer.finished·object.operation_finished를 같은 단위로 합산하지 않는다.
HTTP 오류·중단은 파일 상태 변경의 실패 확정이나 클라이언트 수신 확인이 아니다.
Activity 테이블·응답·보존 정책과 배치 집계 저장은 이번 수집 계약과 별개다.

## 콘솔 조회

Activity는 PostgreSQL의 감사·명령 호출·보안 이력을 읽는다. 명령 호출은 결과와
소요 시간을 목록에 표시하며 unknown을 성공이나 실패로 바꾸지 않는다. 상세의
Operation ID는 관리 이력의 request_id이며 운영 로그의 attributes 안에 있는
grove.operation_id와 연결된다. DB 필드와 Activity 응답은 변경하지 않는다.
전송·worker 로그는 Activity에 복제하지 않고 Loki에서 조회한다.
명령 목록은 대상과 마지막 처리 단계를, 보안 목록은 검사 단계와 기록 방식을 표시한다.
상세는 인증 방식과 작업 ID를 함께 제공한다. 과거 행의 빈 맥락은 Not recorded,
기록 방식은 Unknown으로 표시한다. Representative는 실제 시도 횟수가 아닌 제한 저장
표시이고, Individual도 전송·저장의 무손실 보장을 뜻하지 않는다.

Operation ID 필터는 세 이력의 request_id를 동일하게 조회한다. 계정·토큰 조건과
기존 역할 범위의 교집합에 적용하며, 커서 페이지네이션을 유지한다. 상세의 관련 이력
링크는 이 조건을 유지하고 같은 작업으로 범위를 좁힌다. 연관 행이 없다는 사실만으로
작업 실패나 기록 유실을 단정하지 않는다.

목록은 읽기 쉬운 표시명을 사용하고 원본 코드는 툴팁·상세의 Technical details에
유지한다. 모바일 명령 목록도 결과와 소요 시간을 함께 표시한다. 세션·토큰·이벤트 ID는
접힌 기술 정보에서 조회한다. API의 원본 코드·이력 저장 형식은 바뀌지 않는다.

## 관리 이력 수집

감사는 확정된 변경을, 명령 이력은 실행 결과를, 보안 이력은 인증·접근 검사를 기록한다.
같은 작업의 이력은 DB `request_id`로 연결한다. 세 종류의 행 수가 같을 필요는 없다.

| 기록 | 수집 대상 | 추가 맥락 |
|---|---|---|
| 감사 | 계정·권한·인증수단·세션·리소스 설정의 확정 변경 | 대상 ID와 허용된 변경 내용 |
| 명령 | 변경 명령, 수동 storage.test, 인증수단 목록 조회, 인증된 호출의 실패·거부·unknown | target_type·target_id·stage·auth_method·duration_ms |
| 보안 | 로그인·재인증 성공/실패, 권한·브라우저 검사 거부, 인증 제한·인증 경로 장애 | operation·stage·auth_method·is_sampled |

일반 목록·상세·현재 세션·사용량·이력 조회의 성공은 명령 테이블에 저장하지 않는다.
수집 정책은 명령의 읽기/변경 구분과 별개다. 예를 들어 storage.test는 읽기지만
명시적인 진단이므로 성공도 저장한다. 스키마 해독 전에 거절되거나 프로세스가 종료된
시도까지 모두 저장하는 계약은 아니다.

stage는 관찰한 마지막 처리 경계다. authentication·authorization·reauthentication·
validation·execution·commit·browser_admission을 사용하며, 내부 DB 함수의 세부
실패 위치를 추측하지 않는다. auth_method는 검사한 password·session·api_token이며
비밀번호·토큰 값이 아니다. 새 계정 ID가 발급되기 전 실패한 생성은 대상 ID가 없다.

익명 보안 실패는 기존 surface·reason 단위의 1분 제한 저장을 유지한다.
is_sampled=true는 대표 기록이라는 뜻이며 실제 시도 횟수나 누락 수가 아니다.
인증된 수행자가 있는 보안 기록은 is_sampled=false다. 기존 행의 새 맥락은 NULL로
유지하며, 과거 인증 방식·단계·제한 저장 여부를 추정하지 않는다.

명령 대상에는 계정·인증수단·세션 UUID 또는 등록 가능한 Storage/Client ID만 넣는다.
서비스 키 변경의 대상은 Client이며 키 값·키 해시를 복제하지 않는다. 새 이름 변경
감사는 name_changed만 기록한다. 기존 감사 원문은 소급 수정하지 않는다.

감사 저장 실패는 해당 DB 변경을 취소한다. 명령·보안 저장은 각각 최대 250ms의
best-effort이며, 시간 초과는 저장 미확인이지 반드시 미저장이라는 뜻은 아니다.
작업 취소·프로세스 종료에 의한 마지막 기록 유실은 남는다. 보존 기간은 기존
감사 365일·명령/보안 90일이며 기간 내 기록도 모든 연관 행의 존재를 보장하지 않는다.

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
| INFO | 정상 서비스 요청 종료·명시된 변경 단계·실제 정리·중단 관찰·느린 정상 요청 |
| WARN | 요청 거부·미달·timeout·재시도 가능한 상세 실패; 미지원 S3 501 |
| ERROR | HTTP 서버 오류·내부 I/O 오류·작업 실패 집계·이력 기록 실패·관리 unknown |

기본 INFO 출력은 정상 DEBUG 진단을 제외하되 서비스 요청 종료는 포함한다.
로그 기반 분석은 http.request_finished의 수집된 요청을 대상으로 한다. 전체 트래픽의
운영 지표는 처리 지점의 메트릭을 유지하며 임의 로그 건수로 대체하지 않는다.

필드 이름과 형식의 allowlist가 DEBUG/TRACE에도 적용된다. payload·raw key·파일명·
presigned URL·auth/header·토큰·계정 이름·IP·자유로운 message/error는 출력하지 않는다.
ID는 UUID, 등록 저장소 ID와 의미 코드는 길이·문자 집합을 제한한다. 의존성 target은
출력 대상이 아니다. 신규 진단은 내부 ID·단계·고정 코드를 추가하고 원문 데이터와
오류 체인을 로그 필드로 사용하지 않는다.

## 출력과 조회

stdout writer는 tracing-appender의 전용 thread와 1,024줄 lossy 큐를 사용한다.
큐 포화는 요청을 기다리게 하지 않는다. `grove_log_dropped_lines_total` 메트릭은
큐 유실의 프로세스 누적값이며 각 로그에 반복하지 않는다. stdout I/O 오류나
collector/Loki 유실까지 세지 않는다.
guard는 정상 종료 시 큐 drain을 시도한다. SIGKILL·출력 정지·비정상 종료에서는
마지막 기록을 보장하지 않는다. [tracing-appender](https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/index.html)

Loki의 기존 workload selector 뒤에서 JSON 필드를 조회한다. 실제 label 값은
수집기의 설정을 따른다. [LogQL JSON parser](https://grafana.com/docs/loki/latest/query/log_queries/#json)

```logql
{namespace="grove-storage"} | json event="event_name", outcome="attributes[\"grove.outcome\"]" | event="grove.transfer.finished" | outcome!="success"
{namespace="grove-storage"} | json event="event_name", outcome="attributes[\"grove.outcome\"]" | event="grove.worker.run_finished" | outcome=~"error|timeout|interrupted"
{namespace="grove-storage"} | json request="attributes[\"grove.request_id\"]" | request="<internal-uuid>"
{namespace="grove-storage"} | json operation="attributes[\"grove.operation_id\"]" | operation="<audit-request-uuid>"
```

管理 이력 보존은 [운영 설정](../stack/README.md#관리-로그-보존)을 따른다.
stdout/Loki 보존은 수집 인프라의 정책이며 관리 DB 보존과 독립이다.
수집기는 severity_text·severity_number를 읽고 기존 level 기반 parser를 교체한다.
error_code나 body에서 수준을 추측하지 않는다. 이전 flat JSON과의 하위 호환은
유지하지 않으므로 수집기·Loki 쿼리·로그 기반 알림을 새 형식에 맞춘 뒤 배포한다.
관리 DB 스키마와 Activity 응답은 유지한다. 메트릭 이름·단위 변경과 조회 전환은
[메트릭 계약](13-metrics.md#조회와-검증)을 따른다.

## 검증

`api/src/logging/tests.rs`는 표준 필드·severity 번호·HTTP 속성 자료형·도메인 접두어,
추적 문맥 미생성, JSON/Pretty 개인정보 제외, ERROR-only 상관관계,
서명 헤더 유지, HTTP 수준, 송수신 EOF·오류·중단·시간 초과, 가상 시간,
worker 단일 종료, 유계 writer 포화·정상 drain을 검증한다.
`management-service/src/logging.rs`의 테스트는 DB 연결을 점유한 상태에서 가상 시간
250ms 제한과 작업 결과 보존을 검증한다. 비밀번호 변경 테스트는 best-effort 호출
이력의 도착을 보장으로 삼지 않고, 같은 request_id의 트랜잭션 감사와 세션 폐기를 확인한다.
전체 API/DB 테스트는 기존 인증·파일 상태·S3 계약의 회귀 범위를 유지한다.
Activity 브라우저 테스트는 대상·단계·인증 방식, 대표/개별/과거 미확인 기록과
밝은/어두운 테마의 모바일·데스크톱 상세를 검증한다. 실제 HTTPS 콘솔 테스트는
PostgreSQL에 저장된 계정 생성·재인증 맥락이 같은 화면과 작업 ID로 연결됨을 확인한다.
실제 수집기·Loki 조회와 MinIO SDK parity는 별도의 E2E/배포 검증이다.
