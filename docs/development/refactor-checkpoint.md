# 리팩토링 중간 평가

기준: 기계적 이관 `0a9c185` 이후 로컬 작업. 릴리스·운영 배포와 구분한다.

이번 검증: workspace 305개 통과·1개 제외, S3·CLI E2E·Clippy·fmt 통과.
후속 계약 검증에서는 동일 SDK 시나리오를 MinIO backend로도 통과했다.
MinIO 실물 바이트·열린 multipart 0개·중지 중 503·재시작 후 읽기를 확인했다.
MinIO Complete 응답 유실 후 기존 객체 보존·실물 관찰 확정·purge·점유 정산도 통과했다.
응답 유실 후 SIGKILL·동일 DB 재시작에서도 소유권 보존·복구·점유 정산을 확인했다.
클라우드 S3/R2, 쓰기 진행 도중 종료·DB 확정 실패, 운영 데이터 이관은 미검증이다.

## 파일 트리

```text
backend/crates/
├── s3-protocol/       S3 규칙: auth·signing·operation·XML·completion·integrity
│   └── tests/        규칙별 독립 테스트 (HTTP·DB·Tokio 의존 없음)
├── object-policy/    공통 검증·파트 계산·ETag·완료 복구 판단
│   └── tests/        순수 입력/결과 검증
├── object-service/   cleanup·native multipart 생성/실패 보상 순서
│   └── tests/        fake 기반 단계별 실패·재시도
├── api/src/
│   ├── s3/          HTTP·인증 조회·규칙 연결·S3 작업 조율
│   ├── v1/          native API·multipart adapter
│   ├── admin/       등록부·운영자 관리·usage
│   ├── reconciler/  native/S3 완료 복구·reclaim 재시도
│   └── spool/       스트림 계측 테스트; 실행 코드는 spool.rs
├── db/              행 락·트랜잭션·위치/lease/세션 보존
│   └── tests/       상태 전이·경합·GC별 PG 통합 테스트
├── infra/           filesystem·외부 S3 I/O
├── core/            설정·암호·해시, 기존 경로 호환 재노출
└── cli/             gscli → 관리자 HTTP API
scripts/
├── e2e-cli.py        격리 DB·서버의 CLI 계약
├── e2e-s3.py         같은 격리 환경에서 S3 검증 조립
├── e2e-s3-recovery.py 완료 응답 유실·실제 Reconciler 복구
├── s3_fault_proxy.py 테스트 전용 Complete 응답 유실 주입
├── s3-capture.py     SDK 기본 객체·multipart·다운로드
└── s3_*_cases.py     인증·multipart·무결성 실패 시나리오
output/              콘솔 HTML 프로토타입·별도 UI 테스트
docs/                현재 계약·구조·검증 범위
```

## 전후 비교

| 항목 | 이관 시점 | 현재 |
|---|---|---|
| crate | core/db/infra/api/cli 5개 | protocol/policy/service 추가, 8개 |
| 규칙 검증 | API·core에 섞임 | DB 없이 직접 실행 |
| API s3/multipart.rs | 750줄 | 679줄, 순수 완료 규칙 제거; I/O 조율은 여전히 큼 |
| API v1/files.rs | 515줄 | 423줄, 생성 보상 흐름 분리 |
| 테스트 배치 | 일부 HTTP 파일 안에 정책 테스트 | protocol 기능별·API auth/spool 별도 파일·SDK 시나리오별 분리 |
| 정리 실패 | 복구 정보 유실 가능 | location·lease 보존과 재시도 |
| S3 계약 | 정상 SDK 흐름 중심 | 잘못된 요청의 오류 코드·기존 데이터 보존·정상 재시도 검증 |

줄 수는 분리 위치의 지표이며 전체 코드 감소나 성능 향상의 증거가 아니다.
이번 작업은 규칙 테스트 비용과 장애 원인 추적 범위를 줄였다. 성능 벤치마크는
실시하지 않았다. UploadPart의 SHA256·CRC32 계측 비용은 추가됐다.

## 평가와 다음 경계

| 영역 | 평가 | 다음 작업 |
|---|---|---|
| SRP | 순수 규칙과 HTTP/DB 경계가 명확해짐 | S3 생성·part·완료 조율은 API에 남아 있어 실패 경계별 검증 후 추출 |
| 서비스 계층 | 정리·native 생성 일부를 독립 실행 | 모든 API 흐름이 서비스로 옮겨진 상태와 구분 |
| DB | 원자적 변경과 락의 소유권 유지 | 파일 크기만 보고 transaction을 나누지 않음 |
| 테스트 | 단위·실제 PG·filesystem/MinIO SDK HTTP·Complete 응답 유실 및 재시작 복구 | DB 확정 실패 검증; 쓰기 진행 도중 종료는 별도 |
| 큰 테스트 | DB 테스트는 현재 최대 298줄 | 독립 fixture/실패 경계가 늘어날 때 분리, 줄 수만으로 crate 추가하지 않음 |
| checksum | PUT/part 요청 무결성 검증 | 저장·조회·전체 multipart checksum·추가 알고리즘은 아직 미완성 |
| 조건부 쓰기 | 미지원 요청은 501, 무시해서 덮어쓰는 동작 제거 | 필요 시 논리키 transaction 안에서 원자적 조건 구현 |
| UI/2차 스토리지 | 콘솔 프로토타입·기존 fs adapter 유지 | 완성 대시보드·스토리지 agent join은 별도 단계 |

현재 결론: 안정화와 책임 분리는 진전됐지만 AWS 전체 호환·운영 검증 완료는 아니다.
MinIO 경유 정상·오류·Complete 응답 유실 및 이후 프로세스 재시작 계약을 확인했다.
다음 우선순위는 DB 확정 실패 뒤 재조정 검증이다. crate 수 증가는 해당 실패 경계에 맞춰 결정한다.
