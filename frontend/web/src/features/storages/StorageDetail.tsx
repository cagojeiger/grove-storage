import { Storage } from "./model";
import { Usage } from "../../api/http";
import { bytes } from "../../design/format";

export function StorageDetail({
  storage,
  usage,
}: {
  storage: Storage;
  usage?: Usage;
}) {
  const fields =
    storage.kind === "fs"
      ? [["루트 경로", storage.root_path]]
      : [
          ["Endpoint", storage.endpoint],
          ["Public endpoint", storage.public_endpoint],
          ["리전", storage.region],
          ["버킷", storage.bucket],
          ["Access key", storage.access_key],
          ["Path-style", storage.force_path_style ? "사용" : "해제"],
          ["릴레이", storage.force_relay ? "사용" : "해제"],
        ];
  return (
    <>
      <section className="storage-section" aria-label="저장소 설정">
        <h2>설정</h2>
        <dl className="detail-fields">
          <div>
            <dt>종류</dt>
            <dd>{storage.kind === "fs" ? "파일시스템" : "S3"}</dd>
          </div>
          <div>
            <dt>등록 용량</dt>
            <dd>
              {bytes(storage.capacity_bytes)}{" "}
              <span className="muted">
                ({storage.capacity_bytes.toLocaleString("ko-KR")} bytes)
              </span>
            </dd>
          </div>
          {fields.map(([label, value]) => (
            <div key={label}>
              <dt>{label}</dt>
              <dd>{value || "-"}</dd>
            </div>
          ))}
        </dl>
      </section>
      <section className="storage-section" aria-label="저장소 점유">
        <h2>점유</h2>
        {usage ? (
          <dl className="usage-fields">
            <div>
              <dt>사용 중</dt>
              <dd>{bytes(usage.active_bytes)}</dd>
              <dd className="muted">
                {usage.active_files.toLocaleString("ko-KR")} 파일
              </dd>
            </div>
            <div>
              <dt>예약</dt>
              <dd>{bytes(usage.reserved_bytes)}</dd>
              <dd className="muted">
                {usage.reserved_files.toLocaleString("ko-KR")} 파일
              </dd>
            </div>
            <div>
              <dt>정리 대기</dt>
              <dd>{bytes(usage.purge_pending_bytes)}</dd>
              <dd className="muted">
                {usage.purge_pending_files.toLocaleString("ko-KR")} 파일
              </dd>
            </div>
            <div>
              <dt>잔여</dt>
              <dd>{bytes(usage.remaining_bytes)}</dd>
            </div>
          </dl>
        ) : (
          <p className="muted">점유 정보가 없습니다.</p>
        )}
      </section>
    </>
  );
}
