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
      ? [["Root path", storage.root_path]]
      : [
          ["Endpoint", storage.endpoint],
          ["Public endpoint", storage.public_endpoint],
          ["Region", storage.region],
          ["Bucket", storage.bucket],
          ["Access key", storage.access_key],
          ["Path-style", storage.force_path_style ? "Enabled" : "Disabled"],
          ["Relay", storage.force_relay ? "Enabled" : "Disabled"],
        ];
  return (
    <>
      <section className="storage-section" aria-label="Storage settings">
        <h2>Settings</h2>
        <dl className="detail-fields">
          <div>
            <dt>Type</dt>
            <dd>{storage.kind === "fs" ? "Filesystem" : "S3"}</dd>
          </div>
          <div>
            <dt>Registered capacity</dt>
            <dd>
              {bytes(storage.capacity_bytes)}{" "}
              <span className="muted">
                ({storage.capacity_bytes.toLocaleString("en-US")} bytes)
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
      <section className="storage-section" aria-label="Storage usage">
        <h2>Usage</h2>
        {usage ? (
          <dl className="usage-fields">
            <div>
              <dt>Active</dt>
              <dd>{bytes(usage.active_bytes)}</dd>
              <dd className="muted">
                Files: {usage.active_files.toLocaleString("en-US")}
              </dd>
            </div>
            <div>
              <dt>Reserved</dt>
              <dd>{bytes(usage.reserved_bytes)}</dd>
              <dd className="muted">
                Files: {usage.reserved_files.toLocaleString("en-US")}
              </dd>
            </div>
            <div>
              <dt>Pending deletion</dt>
              <dd>{bytes(usage.purge_pending_bytes)}</dd>
              <dd className="muted">
                Files: {usage.purge_pending_files.toLocaleString("en-US")}
              </dd>
            </div>
            <div>
              <dt>Remaining</dt>
              <dd>{bytes(usage.remaining_bytes)}</dd>
            </div>
          </dl>
        ) : (
          <p className="muted">Usage is unavailable.</p>
        )}
      </section>
    </>
  );
}
