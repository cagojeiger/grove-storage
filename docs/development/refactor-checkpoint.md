# 리소스 리팩토링 마감

기준: `520e172`까지의 코드·로컬 검증. S3-only 중개/릴레이와 NoteGate 호환 범위를 마감한다.
릴리스·운영 이관·배포는 별도 단계다.
아래 migration 번호는 당시 기록이다. 현재 새 설치 기준은 [새 DB baseline](fresh-installation.md)이다.

## 책임 경계

```text
Console / CLI / MCP
    -> Management       설정·권한·안전장치·변경 감사
        -> PostgreSQL   Storage·Client·키·metadata

Native / S3 API
    -> Object Service   설정 조회·파일 수명주기·복구 조율
        -> PostgreSQL   위치·lease·완료 상태·사용량
        -> Transfer     외부 S3 I/O·전송용 임시 스풀

Presigned bytes: Client <-> External S3
Relay bytes:     Client <-> Grove <-> External S3
```

논리적 책임 구분이다. 같은 서버·DB에서 실행하며, 설정 관리에는 화면의 Resources도 포함된다.
Object Service는 파일 상태를 쓰는 control-plane 책임이며 Transfer가 바이트 경로를 담당한다.

## 전후

| 경계 | 이전 | 현재 | 근거 |
|---|---|---|---|
| 설정 관리 | 전송 표면별 처리·응답 중복 | 공통 명령·서비스·자격증명·usage 응답 | `1395328`, `6042076` |
| 등록과 I/O | 설정 처리와 backend 연결 혼재 | `storage_registration` / `storage_access` | `3f2a23e` |
| Native 완료 | HTTP 처리 안에서 완료 조율 | `object-service/single_commit`, `multipart_commit` | `80cb22a`, `f68b26a` |
| 저장소 | FS·S3 분기와 root_path | S3-only, FS adapter 제거 | `dc23721`, `0581766` |
| 전송 임시 파일 | FS 저장소와 관련 구현 혼재 | `infra/temp_spool`, 전송 중 버퍼만 담당 | `0581766` |
| 조건부 PUT | NoteGate create-only 요청 미지원 | 단일 PUT `If-None-Match: *`, 경합·복구 처리 | `019ff79` |
| 소비자 검증 | Grove 내부 계약 중심 | 실제 NoteGate 핸들러·브라우저 연결 | `f9458ff`, `520e172` |

현재 workspace는 11개 crate다. crate 수·줄 수는 성능 개선 지표와 구분한다.
파일 배치는 [소스 구조](source-layout.md), 상세 계약은 [등록부](../spec/01-registry.md)를 따른다.

## DB 변경

| Migration | 의미 | 이관 조건 |
|---|---|---|
| `0015` | Storage·Client JSON metadata | 등록 정보이며 파일 본문과 별개 |
| `0016` | 관리 신원은 `management.accounts`로 통합 | 이전 writer 중단; 리소스 리팩토링과 별도 변경 |
| `0017` | S3-only 제약·root_path 제거 | FS 행이 있으면 migration 차단; 자동 데이터 변환과 구분 |
| `0018` | `s3_uploads.if_none_match` 추가 | 기본 false로 기존 PUT 의미 보존, 조건부 완료·복구에 사용 |

Native API·S3 논리키·업로드 복구 계약을 유지한다. 호환 DTO의 `root_path=null`은 DB 열이 아니다.

## 검증 근거

아래는 앞선 단계의 실행 결과다. 문서 정리를 새 제품 테스트 실행으로 계산하지 않는다.

| 실행 | 확인 범위 |
|---|---|
| Rust workspace·Clippy·fmt | 회귀 통과; CLI native-release-artifact 테스트 1개 기존 제외 |
| MinIO SDK·Native direct/relay | 단일·multipart·실물 바이트·실패·재시도 |
| 완료 복구 | 응답 유실·재시작·DB 커밋 거부·조건부 PUT 승자 보존 |
| NoteGate 핸들러 27개 | 실제 Grove·MinIO 연결, REST/MCP 업로드 계약 |
| NoteGate 브라우저 | 로컬 테스트 OIDC·세션·43 KB 단일·101 MiB multipart·다운로드 바이트/SHA-256 |

재현 명령과 한계는 [S3 호환성 점검](s3-compatibility-review.md)에 모은다.
NoteGate 업로드 직후 사용량 표시 캐시 지연은 별도 UI 이슈로 남는다.

## 다음 단계

| 항목 | 상태·완료 기준 |
|---|---|
| 리소스 코드 구조 | 현 범위 마감; S3 multipart 조율 일부는 API에 유지 |
| Management | [준비 계획](management-review.md) 순서로 계약 점검·측정 후 최소 변경 |
| 새 설치·복원 | [격리 초기화·복원 리허설](fresh-installation.md); 기존 DB 이관은 별도 |
| 운영 연결 | AWS/R2·실제 OIDC·Ingress/TLS·운영 endpoint 별도 검증 |
| 추가 장애 | 쓰기 진행 중 종료·DB COMMIT 응답 유실 별도 검증 |
| 미래 기능 | 자동 배치·S3 간 이동·독립 filesystem Node/Agent 조인 별도 설계 |

추가 crate나 서버 분리는 재현된 결함·중복·변경 비용을 근거로 결정한다.
