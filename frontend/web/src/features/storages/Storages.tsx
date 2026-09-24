import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  ChevronRight,
  HardDrive,
  Pencil,
  Plus,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { admin, message, request, Usage } from "../../api/http";
import { storageLink } from "../../app/navigation";
import { bytes } from "../../design/format";
import { Storage, refreshStorages } from "./model";
import { StorageEditor } from "./StorageEditor";
import { DeleteStorage } from "./DeleteStorage";
import { StorageDetail } from "./StorageDetail";

export function Storages({ route }: { route: string }) {
  const cache = useQueryClient();
  const [dialog, setDialog] = useState<"edit" | "delete" | null>(null);
  const [search, setSearch] = useState("");
  let id = "";
  try {
    id = decodeURIComponent(route.slice("storages/".length));
  } catch {
    id = "";
  }
  const list = useQuery({
    queryKey: ["storages", "list"],
    enabled: !id,
    queryFn: ({ signal }) =>
      request<Storage[]>(`${admin}/storages`, { signal }),
  });
  const detail = useQuery({
    queryKey: ["storages", "detail", id],
    enabled: Boolean(id),
    queryFn: ({ signal }) =>
      request<Storage>(`${admin}/storages/${encodeURIComponent(id)}`, {
        signal,
      }),
  });
  const usage = useQuery({
    queryKey: ["storage-usage"],
    queryFn: ({ signal }) => request<Usage[]>(`${admin}/usage`, { signal }),
  });
  const current = id ? detail : list;
  const refreshing = current.isFetching || usage.isFetching;
  const rows = list.data?.filter((storage) =>
    storage.id.toLowerCase().includes(search.toLowerCase()),
  );
  function close() {
    setDialog(null);
  }
  function showList() {
    close();
    window.location.hash = "storages";
  }
  return (
    <main className="overview storages">
      {id && (
        <a className="back-link" href="#storages">
          <ArrowLeft size={16} />
          저장소
        </a>
      )}
      <div className="page-heading">
        <div>
          <p className="eyebrow">REGISTRY</p>
          <h1>{id || "저장소"}</h1>
        </div>
        <div className="page-actions">
          <button
            className="icon-button"
            title="새로고침"
            aria-label="새로고침"
            disabled={refreshing}
            onClick={() => void refreshStorages(cache)}
          >
            <RefreshCw size={17} className={refreshing ? "spin" : ""} />
          </button>
          {id ? (
            <>
              <button
                className="icon-button"
                title="저장소 수정"
                aria-label="저장소 수정"
                disabled={!detail.data || current.isError}
                onClick={() => setDialog("edit")}
              >
                <Pencil size={17} />
              </button>
              <button
                className="icon-button danger"
                title="저장소 삭제"
                aria-label="저장소 삭제"
                disabled={!detail.data || current.isError}
                onClick={() => setDialog("delete")}
              >
                <Trash2 size={17} />
              </button>
            </>
          ) : (
            <button
              className="primary action-button"
              onClick={() => setDialog("edit")}
            >
              <Plus size={17} />
              등록
            </button>
          )}
        </div>
      </div>
      {current.isPending ? (
        <p className="empty" role="status">
          저장소 조회 중...
        </p>
      ) : current.isError ? (
        <p className="query-error" role="alert">
          {message(current.error)}
        </p>
      ) : id && detail.data ? (
        <>
          {usage.isError && (
            <p className="query-error" role="alert">
              점유 정보를 조회하지 못했습니다. {message(usage.error)}
            </p>
          )}
          <StorageDetail
            storage={detail.data}
            usage={
              usage.isError
                ? undefined
                : usage.data?.find((row) => row.storage_id === id)
            }
          />
        </>
      ) : (
        <>
          <div className="list-toolbar">
            <label>
              <span className="sr-only">저장소 검색</span>
              <input
                type="search"
                placeholder="저장소 검색"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
              />
            </label>
            <span className="muted">{list.data?.length ?? 0}개</span>
          </div>
          <div className="registry-list">
            <div className="registry-labels" aria-hidden="true">
              <span>저장소</span>
              <span>주소</span>
              <span>등록 용량</span>
              <span />
            </div>
            {rows?.map((storage) => (
              <a
                className="registry-row"
                key={storage.id}
                href={storageLink(storage.id)}
              >
                <div className="storage-name">
                  <HardDrive size={18} />
                  <div>
                    <h2>{storage.id}</h2>
                    <span className="muted">
                      {storage.kind === "fs" ? "파일시스템" : "S3"}
                    </span>
                  </div>
                </div>
                <span className="storage-address">
                  {storage.kind === "fs" ? storage.root_path : storage.endpoint}
                  {storage.kind === "s3" && (
                    <span className="muted">{storage.bucket}</span>
                  )}
                </span>
                <span className="storage-size">
                  {bytes(storage.capacity_bytes)}
                </span>
                <ChevronRight size={16} />
              </a>
            ))}
            {rows?.length === 0 && (
              <p className="empty">
                {search ? "검색 결과가 없습니다." : "등록된 저장소가 없습니다."}
              </p>
            )}
          </div>
        </>
      )}
      {dialog === "edit" && (
        <StorageEditor
          storage={id ? detail.data : undefined}
          onClose={close}
          onSaved={(saved) => {
            close();
            window.location.hash = storageLink(saved);
          }}
        />
      )}
      {dialog === "delete" && (
        <DeleteStorage id={id} onClose={close} onReturnToList={showList} />
      )}
    </main>
  );
}
