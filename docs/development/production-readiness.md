# 릴리스 준비

기준: 2026-10-07 작업 트리. 현재 후보는 새 DB 설치용 Grove Storage다.
FileGate/이전 Grove DB의 인플레이스 업그레이드와 운영 이관은 이 범위에서 제외한다.
작업 트리, 후보 커밋, CI, 발행 산출물, 실제 배포는 각각 다른 검증 상태다.

## 변경 경계

| 대상 | 현재 계약 |
|---|---|
| 실행 파일 | 서버 `grove-storage`, 원격 CLI `gscli` |
| 설정 | `GROVE_*`; `FILEGATE_*` 별칭 없음 |
| 내부 crate | `grove-*` |
| DB | 23개 테이블·SQL baseline 4개; 기존 이력과 checksum 불일치 시 거절 |
| Native / S3 | 기존 HTTP 경로·SigV4·presigned 전송·완료/복구 계약 유지 |
| 인증 | Account는 관리 영역, Client 서비스 키는 파일 영역 |
| 이미지 | 서버와 콘솔 dist를 포함하는 한 종류의 이미지; `gscli`는 별도 자산 |
| 버전 | `VERSION`은 `0.5.0`; CI와 발행 산출물 검증 후 릴리스 |
| 운영 환경 | 이번 변경에서 운영 DB·S3·클러스터를 조회하거나 변경하지 않음 |

## 후보 검증

```text
범위 검토 -> 후보 커밋/PR -> 같은 SHA의 CI
  -> 최신 main 반영 -> 새 버전 발행
  -> 이미지 digest / CLI checksum / 콘솔 자산 확인
  -> 별도 승인된 배포와 운영 endpoint 검증
```

| 검증 | 검증 기준 |
|---|---|
| 소스 정합성 | Rust check/fmt/Clippy, crate 경계, 미사용 의존성, 버전 검사 |
| 새 DB | 초기화·재실행·제약·checksum 거절·DDL rollback 테스트 |
| 파일 계약 | MinIO 표준 S3 SDK·presigned PUT/GET·multipart·복구·실물 바이트 |
| 관리 계약 | CLI/MCP·비밀번호·세션·권한·감사 원자성·로그 보존 |
| 브라우저 | FE lint/build·Playwright·실제 HTTPS/cookie/CSRF 계약 |
| 복원 | [새 설치·백업 복원](fresh-installation.md)의 행/checksum·키·미완료 업로드 검증 |
| 이미지 | amd64/arm64 빌드·non-root/read-only·패키징 콘솔·취약점/Secret 검사 |
| 발행 자산 | main/tag SHA, image digest, CLI version/checksum 대조 |

과거 체크포인트의 테스트 수와 CI 성공은 당시 revision의 기록이다.
최종 후보 검증이나 발행 이미지 검증을 대체하지 않는다.
현재 작업 트리의 로컬 실행 결과는 [새 설치 검증](fresh-installation.md#local-verification)에
기록한다. 후보 이미지 보안 검사와 발행 자산 확인은 아직 별도 검증 대상이다.

## 운영 구성

- 초기 Admin은 로컬 대화형 `account init`으로 설정하며 기본 비밀번호는 없다.
- 암호화 root/key ID는 외부 Secret으로 공급한다. DB 백업과 별도로 보호한다.
- 콘솔은 전용 HTTPS 관리 origin을 사용한다. 관리 호스트는 사용자 객체를 제공하지 않는다.
- 프로세스당 DB 연결 예산은 주 풀 상한 + 관리 정리 전용 최대 2개다.
- Object reconciler와 관리 로그 정리는 같은 프로세스 안의 독립 작업·잠금이다.
- 자동 배치·S3 간 이동·OIDC·독립 filesystem Node는 현재 배포의 선행 기능이 아니다.

## 복원 한계

DB 복원은 동일 스키마와 대응하는 바이너리·암호화 키를 기준으로 한다.
PG 백업은 외부 S3 바이트를 포함하지 않으며 백업 이후의 객체 삭제/변경을 되돌리지 않는다.
이미 발급된 direct S3 URL은 서버 중단 뒤에도 만료 전까지 사용될 수 있다.
기존 FileGate 데이터 가져오기와 서비스 중단 계획은 별도 작업이다.

이미지와 CLI의 상세 발행 계약은 [릴리스](releases.md), 보안 게이트는
[이미지 보안](image-security.md), 실행 설정은 [기술·운영](../stack/README.md)을 따른다.
