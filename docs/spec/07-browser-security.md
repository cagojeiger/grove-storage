# spec 07: 관리 브라우저의 보안 경계

- 상태: 프론트엔드 리다이렉트 차단·로컬 보안 헤더 구현. 운영 호스팅·ID/비밀번호 로그인 검증은 후속.
- 선행 계약: [관리자 인증](05-admin-auth.md), [콘솔](06-console.md).

## Origin 분리

```text
console.example.com               data.example.com
├── /api/admin/console/           ├── /{bucket}/{key}
├── /api/admin/v1/                ├── /blobs/...
└── /readyz                       └── /api/v1/...
    나머지 경로: 404                  업로드·다운로드 경로
```

콘솔과 관리 API는 같은 HTTPS origin을 사용한다. 사용자 파일·S3·relay는 별도 호스트로
제공한다. 같은 서버 프로세스를 사용해도 reverse proxy가 호스트별 경로를 제한한다.
관리 호스트의 정적 root에는 콘솔 빌드 산출물만 둔다.

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
| 여러 탭·뒤로 가기 | logout/비밀번호 변경 통지, 복원 시 세션 재검증 후 비공개 화면 표시 | 인증 전환 시 구현·검증 |
| 일회성 토큰 | 발급 직후 한 번 표시, 닫기·세션 종료 시 제거 | User/Agent 토큰 UI 구현 시 검증 |

## 문서 응답 헤더

정본은 [security-headers.mjs](../../frontend/web/security-headers.mjs)의 기본 정책이다.
`scripts/preview.mjs`는 빌드 화면에 이 정책을 적용한다. Vite 개발 모드는 React refresh·
스타일 삽입·HMR용 inline script/style 및 websocket을 추가 허용한다. 운영은 기본 정책을 쓴다.

| 헤더 | 기본 정책 |
|---|---|
| Content-Security-Policy | default none; script/style/img/font/connect self; base none; form self; frame-ancestors none |
| X-Frame-Options | DENY |
| X-Content-Type-Options | nosniff |
| Referrer-Policy | no-referrer |
| Permissions-Policy | camera·microphone·geolocation 비활성 |
| Cache-Control | 콘솔 HTML·관리 응답 no-store |
| Strict-Transport-Security | 운영 TLS 앞단에서 설정; 적용 호스트 범위를 검증한 뒤 활성화 |

헤더는 정적 HTML 응답에 실려야 한다. API 응답에만 CSP를 붙이거나 HTML meta만 사용하는
것으로 frame-ancestors를 대신할 수 없다. 운영 ingress의 실제 응답 헤더를 배포 후 확인한다.
로컬 HTTP 샘플 서버는 인증을 모사하므로 실제 쿠키·비밀번호 인증의 검증 근거는 HTTPS fixture다.

## 비밀번호 로그인 완료 조건

| 기능 | 구현·검증 조건 |
|---|---|
| 최초 설정 | 설치 권한으로 단일 Owner 생성; 동시 초기화 테스트 |
| 로그인 | 공통 실패 응답·공유 rate limit; 인증 성공 시 새 세션 발급 |
| 폼 | username/current-password/new-password autocomplete 구분; 자동 비밀번호 생성·붙여넣기 지원 |
| 변경 | 현재 비밀번호 재확인, 변경 트랜잭션 이후 기존 브라우저 세션 폐기·재로그인 |
| 분실 복구 | 로그인 화면에 로컬 복구 방법 안내; 복구 후 관리 세션·User/Agent 토큰 폐기 |
| 세션 관리 | 목록·현재 세션 표시·개별/전체 종료; 세션 원문 조회 대신 공개 ID 사용 |
| 권한 | User/Agent 판정은 서버에서 집행; UI 숨김과 독립적으로 API 거부 검증 |

## 검증과 남은 범위

| 검증 | 상태 |
|---|---|
| 307/308가 로그인 token body를 재전송하는 현상 | 수정 전 재현, 수정 후 후속 요청 0건 |
| 빌드 화면 인라인 script·외부 connect·iframe 차단 | `tests/browser-security.spec.ts` 실제 Chromium 검증 |
| 로그인·401·secret 제거·CRUD·반응형 | 기존 Playwright suite에 함께 실행 |
| 파일 HTML을 관리 origin에서 열 수 있는 배포 조건 | 코드·계약에서 확인한 조건부 위험; 운영 ingress는 이번 점검 범위 밖 |
| 운영 호스트 경로 제한·TLS·응답 헤더 | 배포 완료 조건; 미검증 |
| 새 비밀번호 로그인·세션 관리·다중 탭 | 인증 전환 구현 단계의 필수 회귀 테스트 |

근거: [OWASP CSP](https://cheatsheetseries.owasp.org/cheatsheets/Content_Security_Policy_Cheat_Sheet.html),
[OWASP 파일 호스트 분리](https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html),
[MDN 리다이렉트 차단](https://developer.mozilla.org/en-US/docs/Web/API/Response/redirected).
