# spec 10: 관리 MCP

- 상태: 로컬 구현·검증. 운영 배포·proxy 검증은 별도.
- 명령 정본: [spec 09](09-management-commands.md). 권한·감사: [spec 08](08-management-plane.md).
- 전송: 공식 Rust SDK `rmcp 3.4.1`, stateless Streamable HTTP.

## 연결

```text
MCP client ── User Bearer ── /api/admin/mcp
                                     │
                         현재 신원 확인 → SDK protocol 처리
                                     │ tools/call
                         공통 command → 현재 권한 재확인
                                     │
                      자원 변경 + 감사 commit → typed 결과
```

| 항목 | 계약 |
|---|---|
| URL | `https://<api-host>/api/admin/mcp` |
| 인증 | `Authorization: Bearer gsm_…`; 콘솔 세션 API에서 발급한 User 토큰 |
| Host | loopback 또는 `FILEGATE_PUBLIC_URL`의 authority; proxy는 허용된 Host를 전달 |
| 브라우저 경계 | Origin이 있으면 403; Cookie·중복 Authorization·master/이전 운영자 토큰은 401 |
| proxy | Bearer를 보존하는 기계용 API 경로; 콘솔 OAuth 로그인 redirect와 분리 |
| MCP 버전 | `2026-07-28` stateless 요청; SDK가 protocol·method/name 헤더와 body를 대조 |
| 이전 연결 흐름 | `2025-11-25` initialize·tools/list 호환 검증; 세션 ID 없이 요청별 인증 유지 |
| 상태 | HTTP 요청마다 신원 확인; 연결 세션·권한 cache 없이 실행기에서 다시 현재 role/owner 검사 |
| HTTP | POST·JSON 응답; GET/DELETE 405, MCP 세션 쿠키·ID 발급 없음 |
| 한도 | 기존 control 경로의 본문 1 MiB·30초 timeout |
| 응답 | `Cache-Control: no-store`, `X-Content-Type-Options: nosniff` |

MCP 클라이언트의 HTTP Bearer 설정에 토큰을 공급한다. `GROVE_TOKEN`은 gscli의 설정이며
모든 MCP 클라이언트가 자동으로 읽는 환경변수는 아니다. 토큰 원문을 연결 URL에 넣지 않는다.

## CLI와 대응

| 경계 | CLI | MCP |
|---|---|---|
| 명령 | 원격 20개 | 동일한 20개 tool; `storage.list`, `client-key.register` 등 이름 그대로 |
| 입력 | 인자/파일 → 공통 typed 입력 | tools/list의 inputSchema → 같은 decode/검증 |
| 권한 | 현재 User·User 역할 | 동일 |
| 실행 | 공통 resource service | 동일 |
| 감사 표면 | `resource_api` | `mcp`; 서버가 확정 |
| 조회 결과 | 공개 DTO·table/JSON | 같은 DTO를 `result`에 포함 |
| 삭제·교체 확인 | TTY 또는 `--yes` | 클라이언트의 도구 승인; mutation/destructive annotation 제공 |
| Native key | 로컬 raw key 파일을 해시 | 해시만 전달 |
| S3 서비스 키 발급 | 원문을 로컬 0600 파일에 저장 | 원문을 호출한 MCP 클라이언트에 일회성 응답 |
| 로컬 기능 | update·파일 입출력·설정 | MCP 도구 범위 밖 |
| 신원·이력 관리 | 콘솔 전용 | 콘솔 전용 |

`credential.create`의 S3 secret은 MCP 응답을 읽는 클라이언트·모델·대화 기록에 노출될 수 있다.
신뢰하는 클라이언트에서 호출하고 해당 기록을 비밀로 관리한다. 서버는 MCP 클라이언트의
파일 권한이나 transcript 보존을 보장하지 않는다. Provider credential을 입력하는 storage
create/replace도 같은 클라이언트 신뢰 경계다.

## 결과와 오류

성공의 `structuredContent`와 호환용 text content는 같은 JSON이다. outputSchema는
배열 결과도 `result` 필드로 감싼 object이며 공통 DTO의 `$defs` 참조를 보존한다.

```json
{"protocol":1,"request_id":"UUID","command":"client.list","result":["notegate"]}
```

| 상황 | 결과 |
|---|---|
| 유효한 실행 성공 | `isError:false`, 위 envelope |
| 서비스 거부·충돌·실패 | `isError:true`, `{protocol,request_id,error:{code,outcome}}` |
| 미지원 tool·잘못된 입력 | JSON-RPC `-32602`, data에 공통 code·`not_applied`; 실행 0회 |
| HTTP 인증 실패 | 401 + Bearer challenge; 로그인 페이지 없이 JSON 오류 |
| 인증 저장소 장애 | 503, 실행 0회 |
| 변경 commit 불명 | `unavailable/unknown`; 자동 재시도 없이 조회·대조 |
| timeout·전송 단절 | 클라이언트가 변경 결과를 불명으로 취급하고 조회·대조 |

MCP protocol 버전과 `protocol:1` 명령 결과 버전은 서로 다른 계약이다.
tool annotation은 클라이언트 힌트이며 서버의 권한·참조 제약을 대체하지 않는다.

## 기록과 검증

| 영역 | 처리·검증 |
|---|---|
| tools/list·discovery | 인증 검사만 수행, 자원 호출 이력 생성 없음 |
| tools/call | 공통 service가 invocation 한 번; 확정 변경 audit는 같은 transaction |
| 입구 인증 실패 | `surface=mcp` 보안 이벤트; 신원이 확인된 거부는 `mcp.connect` 실패 기록 |
| Origin·protocol 거부 | HTTP 진단 범위; 자원 변경·감사 생성 없음 |
| SDK 로그 | payload가 포함될 수 있는 `rmcp` target 전체를 항상 제외; RUST_LOG보다 우선 |
| 운영 관찰 | 기존 request.end와 공통 command_invocations·audit_events·security_events |
| 순수 테스트 | 20개 schema·이름·annotation·출력 참조, SDK 로그 차단 |
| PG HTTP 테스트 | CLI API 결과 대조·권한·owner·폐기·경계·감사 rollback·unknown |
| 실제 프로세스 | `scripts/e2e-mcp.py`: 20개 도구·CLI 결과·토큰별 audit·폐기·비밀 없는 서버 로그 |

로컬 E2E는 MCP HTTP 요청과 실제 서버/PG를 사용한다. 외부 MCP 앱 연결·운영 TLS·OAuth2
Proxy·S3 Provider 실제 전송은 별도 검증이다.

참고: [공식 Rust SDK](https://github.com/modelcontextprotocol/rust-sdk),
[MCP HTTP 전송 계약](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http).
