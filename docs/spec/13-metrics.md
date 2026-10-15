# 운영 메트릭

## 경계

지표는 요청·스트림·worker의 처리 지점에서 직접 갱신한다. 로그 필터·유실과
독립이며 PostgreSQL 감사 이력의 트랜잭션을 대체하지 않는다. 프로세스별
레지스트리를 주입하고 재시작 시 counter가 초기화된다. scrape 사이에 프로세스가
끝나면 마지막 증분은 수집되지 않을 수 있다.

`GROVE_METRICS_BIND`가 설정되면 별도 리스너의 `/metrics`가 OpenMetrics 1.0을
제공한다. 미설정 시 비활성이다. S3·console 리스너에 지표 경로를 추가하지 않는다.
수집 리스너는 인증이 없으므로 내부 수집망에서만 접근한다. scrape는 DB 질의나
외부 S3 I/O를 실행하지 않는다.

## 지표

모든 이름은 `grove_`로 시작한다. Counter에는 `_total`이 붙고 histogram에는
`_bucket`, `_sum`, `_count`가 붙는다. 소요 시간은 Tokio monotonic 시계의 초다.

| 이름 | 종류 | 라벨·의미 |
|---|---|---|
| `http_requests` | counter | surface·method·route·status_class·outcome; 응답 생성 또는 중단 |
| `http_request_duration_seconds` | histogram | surface·method·route; 응답 생성/중단까지의 시간 |
| `http_requests_in_flight` | gauge | surface·method·route; 응답 생성 전 요청 |
| `transfers` | counter | direction·outcome; relay 스트림의 단일 종료 결과 |
| `transfer_observed_bytes` | counter | direction; 청크 관찰마다 갱신하는 바이트 |
| `transfer_duration_seconds` | histogram | direction·outcome; 스트림 관찰 종료까지의 시간 |
| `transfers_in_flight` | gauge | direction; 아직 종료를 관찰하지 않은 스트림 |
| `worker_runs` | counter | worker·task·outcome; 실행·오류·timeout·skip·중단 |
| `worker_duration_seconds` | histogram | worker·task; 호출 종료까지의 시간 |
| `worker_runs_in_flight` | gauge | worker·task; 실행 중 호출 |
| `worker_last_success_timestamp_seconds` | gauge | worker·task; 실패 없이 완료한 마지막 UTC Unix 시각, 0은 미완료 |
| `worker_processed` | counter | worker·task; 완료 report의 확정 처리·삭제 수 |
| `worker_failures` | counter | worker·task; 완료 report의 실패 관찰 수, 고유 파일 수 아님 |
| `worker_skipped` | counter | worker·task; 재시도 보류 항목·소유권 시도 수 |
| `db_pool_connections` / `db_pool_idle_connections` / `db_pool_max_connections` | gauge | pool=requests/maintenance/observations; 해당 프로세스 풀의 현재 상태 |
| `spool_reserved_bytes` / `spool_capacity_bytes` | gauge | live transfer의 MiB 단위 예약량·예산; 실제 디스크 사용량 아님 |
| `spool_transfers_in_flight` | gauge | 현재 admission slot 점유 수 |
| `log_dropped_lines` | counter | stdout 큐 포화 유실; writer I/O·collector·Loki 유실은 제외 |
| `build_info` | info | 실행 중 패키지 version |
| `database_snapshot_timestamp_seconds` | gauge | 마지막 완전한 DB 수집의 DB 시각; 0은 미수집 |
| `object_backlog_items` | gauge | queue; 복구·정리를 기다리는 파일 수, retry backoff 포함 |
| `object_backlog_oldest_waiting_timestamp_seconds` | gauge | queue; 가장 오래된 대기 기준 시각, 빈 큐·미기록 시각은 시계열 없음 |
| `retention_overdue_rows` | gauge | stream; 기존 보존 삭제 조건을 만족하는 행 수 |
| `retention_oldest_overdue_timestamp_seconds` | gauge | stream; 삭제 대상의 가장 오래된 보존 기준 시각, 대상 없으면 0 |
| `retention_days` | gauge | stream; 적용 중인 보존 기간 |
| `db_table_data_bytes` / `db_table_index_bytes` | gauge | schema·table; 데이터(TOAST 포함)·인덱스의 할당 바이트 |
| `db_table_estimated_rows` | gauge | schema·table·state=live/dead; PostgreSQL 행 수 추정치 |

HTTP 2xx는 파일 전송 완료가 아니다. `send` 바이트는 HTTP 런타임에 제공한 양이며
수신자의 저장이나 TCP ACK를 보장하지 않는다. `receive` 성공은 스풀 쓰기·flush
완료다. 공급자 저장과 DB commit 성공은 아니다. 직접 Client↔외부 S3 전송은
Grove가 관찰하지 않는다.

HTTP outcome은 success/rejected/error/interrupted다. S3 미지원 501은 rejected이며
status_class=5xx를 유지한다. 서비스 오류 집계는 outcome=error를 사용한다.
transfer outcome은 success/incomplete/error/interrupted/timeout/too_slow/rejected다.
실패·중단도 관찰한 지연과 바이트에 포함된다.

worker skip은 마지막 성공 시각을 갱신하지 않는다. object/pass는 스케줄 회차이며
하위 작업의 관찰 실패도 실패로 반영한다. pass와 개별 task의 실패 수를 합산하면
중복 집계된다. timeout·중단의 부분 작업 수는 확정하지 않는다. 여러 Pod의
마지막 성공은 worker·task별 최댓값으로 판단하고 counter는 Pod별 rate를 합산한다.

## 조회와 검증

route는 MatchedPath 템플릿이다. raw path·query·객체 키·파일명·계정·요청/파일
UUID·credential을 라벨에 넣지 않는다. method·surface·outcome과 task는 내부의
유계 어휘를 사용한다. 알려진 전송 결과와 worker baseline은 0으로 초기화한다.
미사용 route가 없다는 것과 scrape 대상이 사라졌다는 것은 다른 상태다.

### DB 주기 수집

메트릭이 활성화되면 부팅 직후와 5분 간격으로 DB snapshot을 수집한다. 전용 풀의
연결 상한은 1이며 요청·관리 정리 풀과 분리된다. read-only repeatable-read
트랜잭션에서 SQL당 2초·잠금 250ms, 연결 획득을 포함한 한 회차는 10초로 제한한다.
시계는 DB 함수와 Tokio 시간으로 대체 가능하며 scrape는 완성된 캐시만 읽는다.
새 테이블·이력 행·별도 보존 정책을 만들지 않는다.

최초 수집 전에는 snapshot 시각 0과 worker 상태만 노출하고 DB 값은 노출하지 않는다.
실패·timeout·중단 시 이전 snapshot 전체를 유지한다. snapshot 시각과
`worker=database,task=snapshot`의 결과로 신선도를 확인한다. 오래된 값은 현재값이
아니다. 여러 Pod가 같은 DB를 수집하므로 DB snapshot 계열은 Pod별 합산하지 않는다.

`database.snapshot_failed`는 run_id와 수집 stage·stream·고정 reason을 기록한다.
stage는 begin/configure/clock/objects/retention/tables/commit이며 stream은 보존
집계에만 있고 나머지는 none이다. reason은 pool_timeout·pool_closed·connection_failed·
lock_timeout·query_canceled·permission_denied·read_only_violation·schema_mismatch·
database_error·decode_failed·collection_failed 중 하나다. SQLSTATE 57014는 원인을
단정하지 않고 query_canceled로 기록한다. 회차 전체 제한은 collection/none/pass_timeout이다.
원문 DB 오류·SQL·값·접속 정보는 보관하거나 출력하지 않는다.

queue는 expired_pending·s3_expired·s3_complete·native_complete·s3_cleanup·
native_cleanup·purge·reclaim_cleanup의 8개다. 정상 진행 중인 open/completing 업로드는
lease가 만료되기 전까지 제외한다. retry backoff는 적체를 숨기지 않는다.
대기 기준은 업로드의 lease 만료 시각, cleanup의 상태 전이(`updated_at`), purge의
삭제 시각이다. `time() - oldest_waiting_timestamp`로 경과 시간을 계산한다.
재시도 시각은 나이를 초기화하지 않는다. reclaimed 전이 시각은 원장에 없어
reclaim_cleanup에는 건수만 제공하며 대기 시간은 추정하지 않는다.

보존 관찰은 실제 정리와 같은 기간·strict cutoff·토큰 참조 보호 조건을 사용한다.
참조 때문에 삭제할 수 없는 토큰은 overdue에 포함하지 않는다. 세션은 만료/취소,
setup은 만료, usage는 UTC 날짜가 보존 기준이다. 이 값은 정리 가능한 잔량이며
모든 만료 데이터의 수는 아니다.

테이블은 고정 allowlist의 22개만 노출한다. 행 수는 누적 통계의 추정치이며
갱신이 지연될 수 있다. 할당 용량은 삭제 직후 줄지 않을 수 있고 WAL·DB 전체·S3
실물 크기는 포함하지 않는다. 증가 속도는 이 gauge의 시간 차로 계산하며 counter
rate를 적용하지 않는다. 정확한 적체/overdue 집계는 데이터량에 따라 비용이 증가한다.
시간 제한을 넘으면 기존 값을 유지하므로 신선도도 함께 판단한다.
DB snapshot은 Pod당 최대 128개 시계열과 수집 worker 계열을 추가한다.

기존 Kubernetes/CNPG 수집은 CPU·메모리·재시작·DB 전체 크기를 담당한다.
provider 개별 호출 계측은 후속 범위다. Grafana 대시보드·알림은 실제 수집된
시계열을 확인한 뒤 추가한다.

`metrics/tests.rs`는 가상 시간, 응답/전송 경계, 취소·EOF·오류, worker 결과,
SQL 없는 scrape, 개인정보·S3 경로 격리를 검증한다. `scripts/e2e-metrics.py`는
격리된 PostgreSQL/MinIO에서 실제 바이너리의 presigned PUT/GET과 지표 증가,
worker 시각, SIGTERM 리스너 종료를 검증한다.
`db/tests/observations.rs`와 `metrics/database/tests.rs`는 실제 PostgreSQL 보존 경계,
참조 보호·backoff·테이블 allowlist·잠금 제한, 가상 주기와 취소·실패 시 캐시 보존을
검증한다. E2E는 실제 바이너리의 DB snapshot과 22개 테이블 용량 노출도 확인한다.
실제 refresh 테스트는 PostgreSQL의 단일 연결을 점유하고 가상 시간 9.999초에는
대기 중이며 10초 제한이 Tokio의 1ms 해상도 안에서 발동함을 확인한다.
timeout·SQL 실패 후 기존 값·성공 시각 유지, inflight 해제,
다음 성공 시 snapshot과 성공 시각 갱신까지 같은 실행 경로로 검증한다.
