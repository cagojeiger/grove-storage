import { useMemo } from "react";
import { Box, Link, useMediaQuery, useTheme } from "@mui/material";
import {
  DataGrid,
  type GridColDef,
  type GridSortModel,
} from "@mui/x-data-grid";
import type { Usage } from "../../api/http";
import { storageLink } from "../../app/navigation";
import { cspNonce } from "../../app/csp";
import type { ResourceListState } from "../../app/resourceList";
import { bytes } from "../../design/format";
import type { Storage } from "./model";

export function StorageGrid({
  rows,
  usage,
  state,
  page,
  total,
}: {
  rows: Storage[];
  usage?: Usage[];
  state: ResourceListState;
  page: number;
  total: number;
}) {
  const desktop = useMediaQuery(useTheme().breakpoints.up("md"));
  const wide = useMediaQuery(useTheme().breakpoints.up("sm"));
  const sortModel = useMemo<GridSortModel>(
    () => [{ field: "id", sort: state.sort === "desc" ? "desc" : "asc" }],
    [state.sort],
  );
  const data = rows.map((row) => {
    const used = usage?.find((item) => item.storage_id === row.id);
    return {
      ...row,
      used: used
        ? used.active_bytes + used.reserved_bytes + used.purge_pending_bytes
        : null,
    };
  });
  const href = state.href;
  const columns = useMemo<GridColDef<(typeof data)[number]>[]>(
    () => [
      {
        field: "id",
        headerName: "Storage",
        flex: 1,
        minWidth: 130,
        renderCell: ({ row, tabIndex }) => (
          <Link tabIndex={tabIndex} href={href(storageLink(row.id))}>
            {row.id}
          </Link>
        ),
      },
      {
        field: "endpoint",
        headerName: "Endpoint",
        flex: 1,
        minWidth: 180,
        sortable: false,
      },
      {
        field: "bucket",
        headerName: "Bucket",
        flex: 1,
        minWidth: 120,
        sortable: false,
      },
      {
        field: "used",
        headerName: "Grove usage",
        type: "number",
        minWidth: 115,
        flex: 0.6,
        sortable: false,
        valueFormatter: (value: number | null) =>
          value === null ? "Unavailable" : bytes(value),
      },
      {
        field: "capacity_bytes",
        headerName: "Configured capacity",
        type: "number",
        minWidth: 165,
        flex: 0.7,
        sortable: false,
        valueFormatter: (value: number) => bytes(value),
      },
    ],
    [href],
  );
  return (
    <Box sx={{ height: 560, width: "100%" }}>
      <DataGrid
        nonce={cspNonce}
        aria-label="Storage"
        rows={data}
        columns={columns}
        disableRowSelectionOnClick
        disableColumnFilter
        disableColumnMenu
        disableColumnSelector
        columnVisibilityModel={{
          endpoint: desktop,
          bucket: desktop,
          capacity_bytes: wide,
        }}
        paginationMode="server"
        rowCount={total}
        pageSizeOptions={[20, 50, 100]}
        paginationModel={{ page: page - 1, pageSize: state.size }}
        onPaginationModelChange={(model) =>
          state.update(
            model.pageSize !== state.size
              ? { size: model.pageSize }
              : { page: model.page + 1 },
          )
        }
        sortingMode="server"
        sortModel={sortModel}
        sortingOrder={["asc", "desc"]}
        onSortModelChange={(model) =>
          state.update({ sort: model[0]?.sort ?? "asc" })
        }
        localeText={{
          noRowsLabel: state.search
            ? "No matching storage."
            : "No storage registered.",
        }}
        slotProps={{
          basePagination: {
            material: { showFirstButton: true, showLastButton: true },
          },
        }}
      />
    </Box>
  );
}
