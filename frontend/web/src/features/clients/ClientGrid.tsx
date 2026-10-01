import { useMemo } from "react";
import { useQueries } from "@tanstack/react-query";
import { Box, Link, useMediaQuery, useTheme } from "@mui/material";
import {
  DataGrid,
  type GridColDef,
  type GridSortModel,
} from "@mui/x-data-grid";
import { command } from "../../api/commands";
import { cspNonce } from "../../app/csp";
import type { ResourceListState } from "../../app/resourceList";
import { bytes } from "../../design/format";
import { Client, ClientUsage, clientLink, totals } from "./model";

type Row = {
  id: string;
  storage: string;
  files: number | null;
  bytes: number | null;
};

export function ClientGrid({
  ids,
  usage,
  state,
  page,
  total,
}: {
  ids: string[];
  usage?: ClientUsage[];
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
  const details = useQueries({
    queries: ids.map((id) => ({
      queryKey: ["clients", "detail", id],
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        command<Client>("client.show", { id }, signal),
    })),
  });
  const rows: Row[] = ids.map((id, i) => {
    const used = usage ? totals(usage, id) : undefined;
    return {
      id,
      storage: details[i].isError
        ? "Unavailable"
        : (details[i].data?.storage_id ?? "Loading..."),
      files: used?.files ?? null,
      bytes: used?.bytes ?? null,
    };
  });
  const href = state.href;
  const columns = useMemo<GridColDef<Row>[]>(
    () => [
      {
        field: "id",
        headerName: "Client",
        flex: 1,
        minWidth: 130,
        renderCell: ({ row, tabIndex }) => (
          <Link tabIndex={tabIndex} href={href(clientLink(row.id))}>
            {row.id}
          </Link>
        ),
      },
      {
        field: "storage",
        headerName: "Storage",
        flex: 1,
        minWidth: 150,
        sortable: false,
      },
      {
        field: "files",
        headerName: "Stored files",
        type: "number",
        flex: 0.7,
        minWidth: 120,
        sortable: false,
        valueFormatter: (value: number | null) =>
          value === null ? "Unavailable" : value.toLocaleString("en-US"),
      },
      {
        field: "bytes",
        headerName: "Stored data",
        type: "number",
        flex: 0.7,
        minWidth: 115,
        sortable: false,
        valueFormatter: (value: number | null) =>
          value === null ? "Unavailable" : bytes(value),
      },
    ],
    [href],
  );
  return (
    <Box sx={{ display: "flex", flexDirection: "column", maxHeight: 560, minHeight: 180, width: "100%" }}>
      <DataGrid
        nonce={cspNonce}
        aria-label="Clients"
        rows={rows}
        columns={columns}
        disableRowSelectionOnClick
        disableColumnFilter
        disableColumnMenu
        disableColumnSelector
        columnVisibilityModel={{ storage: desktop, files: wide }}
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
            ? "No matching clients."
            : "No clients registered.",
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
