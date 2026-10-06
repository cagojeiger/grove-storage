# 운영 전환 준비

기준: 2026-10-05. FileGate에서 Grove Storage로 전환하기 위한 릴리스 게이트다.
작업 트리 검증, main CI, 발행 산출물, 실제 배포, 운영 이관은 별도 상태로 관리한다.
이 문서는 운영 변경이나 서비스 중단을 승인하는 실행 스크립트가 아니다.

## 현재 기준점

| 대상 | 확인한 상태 |
|---|---|
| 원격 main | `a5969b3cc5534fb846a8839056ea51d6de812974`; fetch 후 확인 |
| main 검증 | [CI 성공](https://github.com/cagojeiger/grove-storage/actions/runs/36856548385), [Release 성공](https://github.com/cagojeiger/grove-storage/actions/runs/36857675406); 위 SHA 기준 |
| 발행 버전 | Grove `v0.4.1`; 기존 태그와 자산 재사용 금지 |
| 작업 브랜치 | `codex/console-from-scratch`, HEAD `3b69ada451873abe3141ba671f4483a5b49e585e`; main보다 6커밋 앞섬 |
| 추가 변경 | FE 교체, 인증·시간 테스트, 관리 로그 보존, Provider 분리 등이 미커밋 상태; 아직 main 또는 발행 이미지에 포함되지 않음 |
| 운영 환경 | 이번 준비에서 클러스터·운영 DB·운영 S3 상태를 조회하거나 변경하지 않음 |

위 SHA와 성공 기록은 날짜가 있는 스냅샷이다. 병합·발행·배포 직전에 다시 확인한다.

## 준비 순서

```text
작업 범위 확정 -> 후보 커밋/PR -> 동일 후보 SHA 검증
  -> 최신 main 병합 -> 동일 main SHA CI -> 새 버전 발행
  -> 이미지 digest / CLI checksum / 콘솔 빌드 확인
  -> 격리 이관·복원 리허설 -> 운영 writer 중단
  -> 백업 / 이관 / frozen 검증 -> 재개 또는 복원
```

| 우선순위 | 작업 | 완료 근거 |
|---|---|---|
| P0: 후보 고정 | 미커밋 소스·테스트·문서를 검토하고 preview 산출물·캐시·비밀을 릴리스 범위에서 제외 | 리뷰 가능한 커밋/PR, 깨끗한 후보 checkout |
| P0: 실패 유계성 | Provider 제어 호출의 명시적 전체 시간 제한과 복구 재시도 공정성 보완 | 느리거나 실패하는 S3 앞에서도 후속 정리 진행; 가상 시간·PG·provider 회귀 |
| P0: 최종 계약 | 후보 SHA에서 Rust/FE·실제 HTTPS 인증·MinIO·NoteGate·오프라인 이관 검증 | 동일 SHA CI 성공; 작업 트리의 과거 통과 결과로 대체하지 않음 |
| P0: 발행 | 미발행 새 버전으로 main을 릴리스하고 산출물 확인 | main SHA, tag SHA, 이미지 digest, CLI checksum/version 대조 |
| P0: 콘솔 배포 | backend 이미지와 별개로 `frontend/web/dist` 제공 및 관리 HTTPS origin 준비 | CSP 응답별 nonce·쿠키·CSRF·host별 경로 제한·API proxy 검증 |
| P0: 이관·복원 | 실제 FileGate 버전/SQL checksum/키 ID/미완료 작업 조사, 격리 backup restore 검증 | 승인된 중단 창과 복원 절차; writer 동시 실행 없음 |
| P1: 운영 관찰 | DB 연결·정리 backlog·오류·spool 디스크·관리 로그 증가량 점검 | 배치 소진·재시도·보존 정책과 자원 한도 관찰 |

Provider 분리만으로 전체 호출 deadline이나 재시도 공정성이 해결되지는 않는다.
바이트 스트리밍에는 크기·유휴 제한이 있으므로 제어 호출의 시간 정책을 무조건
전체 다운로드 제한으로 확장하지 않는다. 재시도 중인 Complete/Delete/Abort의 결과는
불확실할 수 있어 timeout 뒤 DB 상태를 성공으로 확정하지 않고 기존 복구 계약을 따른다.

## 유지할 운영 계약

- 서버 이름 `filegate`, `FILEGATE_*`, Native/S3 API와 NoteGate 인터페이스를 유지한다.
- Storage/Client ID, 기존 키·객체 주소, 암호화 root/key ID를 보존한다. 외부 S3 객체 이동은 이번 이관 범위가 아니다.
- 등록부 설정과 파일 수명주기 책임을 분리하며 관리 Account를 파일 요청 인증에 추가하지 않는다.
- DB 연결 상한은 프로세스당 주 풀 설정 + 관리 정리 전용 최대 2개다. replica 수와 함께 예산을 계산한다.
- 기본 비밀번호나 개발용 암호화 키를 운영에 쓰지 않는다. 기존 자격증명·암호화 키 교체는 별도 절차로 한다.
- 관리 origin에서는 사용자 객체를 제공하지 않는다. backend Dockerfile은 콘솔 dist를 포함하지 않는다.
- 관리 DB migration 0024는 상수 Account 종류와 과거 Master/Root 인증 구조를 제거한다.
  정상 token/password 세션·감사 기록·S3 리소스는 보존하며 모든 이전 DB writer 중단이 필요하다.
  [인증 스키마 업그레이드](../spec/11-local-management-auth.md)의 백업·복원 조건을 따른다.

## 중단·롤백 경계

NoteGate 중단만으로 쓰기 동결이 완료되지는 않는다. FileGate/Grove의 reconciler와
다른 클라이언트 writer, 이미 발급된 presigned URL을 함께 조사한다. S3 direct URL은
애플리케이션을 중단해도 만료 전까지 유효할 수 있다. 실제 배포 버전과 endpoint의
경로를 확인하고 만료 대기 또는 검증된 쓰기 차단으로 quiescence를 확보한다.

이관 후에는 writer를 재개하기 전에 기존 키와 URL로 읽기, 계정 로그인, DB 행과
객체/파트 보존을 검증한다. 이 구간의 실패는 원본 DB 백업 복원 + 구버전 실행으로
되돌린다. 업그레이드 DB에 구버전 이미지만 실행하는 rollback은 사용하지 않는다.
쓰기 재개 뒤에는 DB와 S3가 달라질 수 있으므로 백업 복원만으로 rollback을 보장하지 않는다.

상세 순서와 한계는 [오프라인 이관 리허설](migration-rehearsal.md)을 따른다.
실제 운영 version이 리허설의 FileGate `v0.4.1`과 다르면 baseline을 다시 검증한다.

## 검증 기록

| 검사 | 근거 수준 |
|---|---|
| Rust 620개 통과, 릴리스 자산 필요 1개 ignore; Clippy·fmt·crate 경계 검사 | 2026-10-05 Provider 분리 후 로컬 작업 트리; 발행 이미지 아님 |
| MinIO S3·Native direct/relay·SIGTERM 종료 통과, 스크립트 24개 통과 | 같은 작업 트리의 격리 fixture; 홈 클러스터 아님 |
| FE lint·production build, 버전 정합성 검사 통과 | 이번 운영 준비에서 재실행; 전체 FE/E2E 재실행 아님 |
| 배포 계약 테스트 26개 중 25개 통과, 1개 skip | native release binary 미지정; 실제 발행 자산 검증 아님 |
| 인증 스키마 0024 적용 후 Rust 623개 통과, 릴리스 자산 필요 1개 ignore | 2026-10-05 격리 PostgreSQL 17; 정상 세션·감사/리소스 보존과 실패 시 transaction rollback 검증 |
| 같은 스키마에서 MinIO S3 SDK·presigned PUT/GET·multipart·중단/복구 통과 | 새 로컬 바이너리와 폐기형 DB/MinIO fixture; 실제 NoteGate나 운영 환경 검증 아님 |
| FileGate v0.4.1 → Grove 이관·백업 복원 및 Grove에서 이전 미완료 업로드 완료 통과; 스크립트 테스트 27개 통과 | 2026-10-05 별도 PG/MinIO 두 fixture; 기존 키·URL·객체·upload ID 보존, migration 24개 checksum 대조; 미커밋 작업 트리이며 발행 이미지 아님 |

최종 후보 고정 이후에는 [릴리스 계약](releases.md)과 [CI](../../.github/workflows/ci.yml)의
전체 게이트를 다시 통과해야 한다. 이번 준비에서는 커밋·push·병합·릴리스·운영 배포를 수행하지 않았다.
