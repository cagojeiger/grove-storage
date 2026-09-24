import { useQuery } from "@tanstack/react-query";
import { RefreshCw, HardDrive } from "lucide-react";
import { admin, message, request, Usage } from "../../api/http";

export function bytes(value: number) {
  const absolute = Math.abs(value);
  const unit =
    absolute >= 1024 ** 4
      ? 4
      : absolute >= 1024 ** 3
        ? 3
        : absolute >= 1024 ** 2
          ? 2
          : absolute >= 1024
            ? 1
            : 0;
  return `${(value / 1024 ** unit).toLocaleString("ko-KR", { maximumFractionDigits: 1 })} ${["B", "KiB", "MiB", "GiB", "TiB"][unit]}`;
}

export function Overview() {
  const query = useQuery({
    queryKey: ["overview"],
    queryFn: async ({ signal }) => {
      const [usage, clients, ready] = await Promise.all([
        request<Usage[]>(`${admin}/usage`, { signal }),
        request<{ id: string }[]>(`${admin}/clients`, { signal }),
        request<{ status: string }>("/readyz", { signal }).then(
          (value) => value.status === "ready",
          () => false,
        ),
      ]);
      return { usage, clients, ready };
    },
    retry: false,
  });
  const data = query.data;
  const sum = (
    key: "active_bytes" | "reserved_bytes" | "purge_pending_bytes",
  ) => data?.usage.reduce((total, row) => total + row[key], 0) ?? 0;
  return (
    <main className="overview">
      <div className="page-heading">
        <div>
          <p className="eyebrow">WORKSPACE</p>
          <h1>개요</h1>
        </div>
        <button
          className="icon-button"
          title="새로고침"
          aria-label="새로고침"
          onClick={() => void query.refetch()}
          disabled={query.isFetching}
        >
          <RefreshCw size={18} className={query.isFetching ? "spin" : ""} />
        </button>
      </div>
      {query.isPending ? (
        <p role="status">불러오는 중...</p>
      ) : query.isError ? (
        <p role="alert">{message(query.error)}</p>
      ) : (
        data && (
          <>
            <div className="status-line">
              <span className={`dot ${data.ready ? "online" : ""}`} />
              <strong>
                API {data.ready ? "준비됨" : "준비 상태 확인 실패"}
              </strong>
              <span className="muted">저장소 연결 상태는 별도</span>
            </div>
            <dl className="metrics">
              <div>
                <dt>저장소</dt>
                <dd>{data.usage.length}</dd>
              </div>
              <div>
                <dt>클라이언트</dt>
                <dd>{data.clients.length}</dd>
              </div>
              <div>
                <dt>활성 데이터</dt>
                <dd>{bytes(sum("active_bytes"))}</dd>
              </div>
              <div>
                <dt>예약 / 삭제 대기</dt>
                <dd className="compact-value">
                  {bytes(sum("reserved_bytes"))} /{" "}
                  {bytes(sum("purge_pending_bytes"))}
                </dd>
              </div>
            </dl>
            <section>
              <div className="section-heading">
                <h2>저장소 점유</h2>
                <span className="muted">{data.usage.length}개</span>
              </div>
              {data.usage.length === 0 ? (
                <div className="empty">
                  <HardDrive size={28} aria-hidden="true" />
                  <p>등록된 저장소가 없습니다.</p>
                </div>
              ) : (
                <div className="storage-list">
                  {data.usage.map((row) => (
                    <article className="storage-row" key={row.storage_id}>
                      <div className="storage-name">
                        <HardDrive size={18} aria-hidden="true" />
                        <div>
                          <h3>{row.storage_id}</h3>
                          <span className="muted">
                            {row.kind.toUpperCase()} · 활성 파일{" "}
                            {row.active_files.toLocaleString()}개
                          </span>
                        </div>
                      </div>
                      <dl className="storage-values">
                        <div>
                          <dt>활성</dt>
                          <dd>{bytes(row.active_bytes)}</dd>
                        </div>
                        <div>
                          <dt>예약</dt>
                          <dd>{bytes(row.reserved_bytes)}</dd>
                        </div>
                        <div>
                          <dt>삭제 대기</dt>
                          <dd>{bytes(row.purge_pending_bytes)}</dd>
                        </div>
                        <div>
                          <dt>남은 용량</dt>
                          <dd
                            className={row.remaining_bytes < 0 ? "danger" : ""}
                          >
                            {bytes(row.remaining_bytes)}
                          </dd>
                        </div>
                      </dl>
                      <div className="capacity">
                        <progress
                          aria-label={`${row.storage_id} 점유율`}
                          max={Math.max(1, row.capacity_bytes)}
                          value={Math.max(
                            0,
                            row.active_bytes +
                              row.reserved_bytes +
                              row.purge_pending_bytes,
                          )}
                        />
                        <span className="muted">
                          등록 용량 {bytes(row.capacity_bytes)}
                        </span>
                      </div>
                    </article>
                  ))}
                </div>
              )}
            </section>
          </>
        )
      )}
    </main>
  );
}
