# spec 07: 관리 브라우저의 보안 경계

- 상태: 콘솔은 로컬 ID·비밀번호 세션을 사용한다. 브라우저 보안 경계는 유지하며 운영 호스팅 검증은 별도다. 이전 master 흐름 기록은 현행 인증 계약이 아니다.
- 현행 로그인·복구·CLI/MCP 권한은 [spec 11](11-local-management-auth.md)을 따른다.
- 콘솔 화면: [spec 06](06-console.md). 이전 운영자 REST 인증: [spec 05](05-admin-auth.md).
- 관리 책임·권한 경계: [spec 08](08-management-plane.md).

## Origin 분리

```text
console.example.com               data.example.com
├── /api/admin/console/           ├── /{bucket}/{key}
├── /api/admin/console-commands/v1 ├── /blobs/...
├── /api/admin/identity/v1/       │   기존 데이터 인증 유지
└── /readyz                       └── /api/v1/...
    나머지 경로: 404                  업로드·다운로드 경로
```

콘솔과 관리 API는 같은 HTTPS origin을 사용한다. 사용자 파일·S3·relay는 별도 호스트로
제공한다. 같은 서버 프로세스를 사용해도 reverse proxy가 호스트별 경로를 제한한다.
관리 호스트의 정적 root에는 콘솔 빌드 산출물만 둔다.

### OAuth2 Proxy와 기계용 API (설계)

| 진입 호스트 예시 | 앞단·서버 인증 | 경로 제한 |
|---|---|---|
| console.example.com | OAuth2 Proxy + Grove 비밀번호 세션 | 콘솔·세션·신원·관리 자원·감사 API |
| api.example.com | User Bearer; 대화형 OAuth 로그인 없음 | 자원 CLI/MCP/API만; 신원·감사·세션 발급 경로 제외 |
| data.example.com | 기존 Client key / SigV4 / 발급 URL | 기존 데이터 경로만 |

호스트 허용 목록뿐 아니라 backend가 자격증명 종류와 작업 권한을 검사한다.
기계용 자원 경로는 콘솔 쿠키를 받지 않아 proxy 보호의 우회 통로가 되지 않는다.
프록시 OIDC 토큰을 Grove의 Authorization으로 주입하지 않고, 전달된 사용자 헤더도
Grove 관리 권한으로 해석하지 않는다. 콘솔에서 호출하는 API는 콘솔 origin을 사용한다.
기계용 401/403은 JSON/MCP 오류로 반환하고 OAuth 로그인 페이지로 redirect하지 않는다.
현재 `/api/admin/commands/v1`은 Bearer 전용 조회·변경 진입점이며 Cookie를 거부한다.
CLI는 이 경로를 사용하고 MCP는 `/api/admin/mcp`의 Bearer 전용 경로를 사용한다.
MCP는 Origin이 있는 요청도 거부한다. UI는 세션 전용 `/api/admin/console-commands/v1`을 사용한다.
이전 `/api/admin/v1` REST 인증은 별도로 유지하며 새 콘솔 프록시 허용 경로에서 제외한다.
SigV4의 Authorization·Host·path·query와 presigned URL은 서명 계약에 맞게 보존한다.
TLS 종료 뒤의 직접 접근 경로·신뢰 proxy 헤더·callback 쿠키·로그아웃을 배포 E2E로 검증한다.

S3는 객체 content type과 서명된 응답 헤더 override를 제공한다. 관리 origin에서
업로드된 HTML을 열면 같은 origin의 스크립트가 관리 쿠키를 동반한 요청을 실행할 수 있다.
HttpOnly·SameSite·CSRF는 같은 origin에서 실행되는 공격 스크립트의 권한을 제거하지 않는다.
콘솔 문서의 CSP도 다른 경로에서 열린 파일 문서에는 전파되지 않는다.

## 브라우저 계약

| 영역 | 계약 | 현재 근거·상태 |
|---|---|---|
| 세션 쿠키 | Secure·HttpOnly·SameSite=Strict·`__Host-`, Domain 없음 | 현행 backend·HTTPS fixture |
| 로그인·변경 | 정확한 Origin + CSRF 헤더, 관리 API CORS 비활성 | 현행 backend |
| 전송 | same-origin 쿠키, no-store, 리다이렉트 오류, 변경 자동 재시도 없음 | `src/api/http.ts`; 307/308 본문 재전송 회귀 테스트 |
| 비밀 | 입력·발급 결과에만 보관; URL·웹 저장소·query/mutation 캐시·로그에서 제외 | 현행 로그인·storage 폼; 신규 토큰 화면에 동일 적용 |
| 렌더링 | 외부 문자열은 React text로 표시 | 현행 화면; 향후 파일 미리보기는 데이터 origin 사용 |
| 세션 종료 | 서버 세션 폐기, 진행 중 조회 취소, 비공개 캐시 제거 | 현행 로그아웃·401 테스트 |
| 여러 탭·뒤로 가기 | logout/토큰 폐기/역할 변경 후 재검증, 복원 시 세션 확인 후 비공개 화면 표시 | 인증 전환 시 구현·검증 |
| 일회성 토큰 | 발급 직후 한 번 표시, 닫기·세션 종료 시 제거 | 일회성 표시·보관 확인·닫기·브라우저 비저장 검증 |

## 문서 응답 헤더

정본은 [security-headers.mjs](../../frontend/web/security-headers.mjs)의 `consoleHeaders(false, nonce)` 정책이다.
HTML 응답마다 암호학적 난수 nonce를 발급하고 `index.html`의 `__GROVE_CSP_NONCE__`를 교체한다.
MUI/Emotion은 같은 nonce로 style 요소를 삽입한다. nonce 없는 inline style/script는 차단한다.
Rust의 [콘솔 제공 경로](../../backend/crates/api/src/console_web.rs)와
`scripts/preview.mjs`는 빌드 화면에 이 정책을 적용한다. Vite 개발 모드는 React refresh·
스타일 삽입·HMR용 inline script/style 및 websocket을 추가 허용한다. 운영은 기본 정책을 쓴다.

| 헤더 | 기본 정책 |
|---|---|
| Content-Security-Policy | default none; script/style/img/font/connect self; style-src-elem self + 응답별 nonce; style-src-attr none; base none; form self; frame-ancestors none |
| X-Frame-Options | DENY |
| X-Content-Type-Options | nosniff |
| Referrer-Policy | no-referrer |
| Permissions-Policy | camera·microphone·geolocation 비활성 |
| Cache-Control | 콘솔 HTML·관리 응답 no-store |
| Strict-Transport-Security | 운영 TLS 앞단에서 설정; 적용 호스트 범위를 검증한 뒤 활성화 |

헤더는 정적 HTML 응답에 실려야 한다. API 응답에만 CSP를 붙이거나 HTML meta만 사용하는
것으로 frame-ancestors를 대신할 수 없다. 운영 ingress의 실제 응답 헤더를 배포 후 확인한다.
로컬 HTTP 샘플 서버는 인증을 모사한다. 실제 인증의 브라우저 근거는
`scripts/e2e-console.py`의 HTTPS·Rust 서버·PostgreSQL fixture와
`scripts/e2e-image.py`의 실제 이미지·새 DB·HTTPS ingress fixture다.
신원 변경은 Admin 비밀번호 세션으로 제한하며 Bearer·cross-origin 요청을 거부한다.
비밀번호 로그인·설정 링크·쿠키·CSRF·폐기·역할 강등은 실제 HTTPS UI에서 검증한다.

## 로컬 비밀번호 인증 계약

| 기능 | 구현·검증 조건 |
|---|---|
| 최초 설정 | 서버 운영자가 `grove-storage account init`으로 첫 Admin 생성; 브라우저는 일반 로그인 사용 |
| 로그인 | 공통 실패 응답·공유 rate limit; 인증 성공 시 새 세션 발급 |
| 폼 | ID·비밀번호 입력·붙여넣기·password manager 지원; 관리 토큰은 CLI/MCP에 사용 |
| 폐기 | 계정 비활성화·역할 변경을 후속 요청에 반영; 관리 토큰과 브라우저 세션은 별도 수명주기 |
| 분실 복구 | 서버의 `grove-storage account recover`; 대상 비밀번호 변경과 세션·관리 토큰 폐기 |
| 세션 관리 | 목록·현재 세션 표시·개별 종료; 비밀번호 변경/로컬 복구 시 전체 폐기; 세션 원문 대신 공개 ID 사용 |
| 권한 | User·role·진입 경계를 서버에서 집행; Admin Bearer의 신원 API 호출도 거부 |
| 설정 링크 | 비밀번호 미설정 계정에 단일 사용 challenge 발급; 로그인 세션과 별개 |

## 검증과 남은 범위

| 검증 | 상태 |
|---|---|
| 307/308 로그인 본문 재전송 방지 | `redirect: error`; 브라우저 회귀 테스트에서 후속 요청 0건 |
| 빌드 화면 인라인 script·외부 connect·iframe 차단 | `tests/browser-security.spec.ts` 실제 Chromium 검증 |
| 로그인·401·secret 제거·CRUD·반응형 | 기존 Playwright suite에 함께 실행 |
| 파일 HTML을 관리 origin에서 열 수 있는 배포 조건 | 코드·계약에서 확인한 조건부 위험; 운영 ingress는 이번 점검 범위 밖 |
| 운영 호스트 경로 제한·TLS·응답 헤더 | 배포 완료 조건; 미검증 |
| 비밀번호 로그인·자원 UI·역할·폐기 | 실제 HTTPS·PG·MinIO fixture; 현재 backend는 S3-only |
| 다중 탭 복원 | 배포 전 세션 재검증·민감 화면 복원 경로를 별도 확인 |
| OAuth2 Proxy·기계용 API 분리 | 배포 전 401/403, Bearer 보존, 신원 API 우회 거부 검증 |

근거: [OWASP CSP](https://cheatsheetseries.owasp.org/cheatsheets/Content_Security_Policy_Cheat_Sheet.html),
[OWASP 파일 호스트 분리](https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html),
[MDN 리다이렉트 차단](https://developer.mozilla.org/en-US/docs/Web/API/Response/redirected).
