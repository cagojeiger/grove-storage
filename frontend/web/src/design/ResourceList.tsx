import { Stack, TablePagination, TextField } from "@mui/material";
import { ResourceListState } from "../app/resourceList";

export function ListToolbar({
  state,
  label,
}: {
  state: ResourceListState;
  label: string;
}) {
  return (
    <Stack
      direction={{ xs: "column", sm: "row" }}
      spacing={2}
      sx={{ my: 3, justifyContent: "space-between" }}
    >
      <TextField
        label={label}
        type="search"
        value={state.search}
        onChange={(event) => state.update({ search: event.target.value })}
        sx={{ maxWidth: { sm: 320 } }}
      />
      <TextField
        label="Sort"
        select
        value={state.sort}
        sx={{ width: { xs: "100%", sm: 160 } }}
        onChange={(event) => state.update({ sort: event.target.value })}
        slotProps={{
          select: { native: true },
          htmlInput: { "aria-label": "Sort" },
        }}
      >
        <option value="asc">Name A–Z</option>
        <option value="desc">Name Z–A</option>
      </TextField>
    </Stack>
  );
}

export function Pagination({
  state,
  page,
  total,
}: {
  state: ResourceListState;
  page: number;
  pages: number;
  total: number;
}) {
  return (
    <TablePagination
      component="div"
      count={total}
      page={page - 1}
      rowsPerPage={state.size}
      rowsPerPageOptions={[20, 50, 100]}
      showFirstButton
      showLastButton
      onPageChange={(_, index) => state.update({ page: index + 1 })}
      onRowsPerPageChange={(event) =>
        state.update({ size: Number(event.target.value) })
      }
      getItemAriaLabel={(type) =>
        `${type[0].toUpperCase()}${type.slice(1)} page`
      }
      slotProps={{
        select: { native: true, inputProps: { "aria-label": "Rows per page" } },
        toolbar: {
          sx: { flexWrap: "wrap", justifyContent: "flex-end", px: 0 },
        },
        spacer: { sx: { display: "none" } },
        displayedRows: { role: "status" },
      }}
    />
  );
}
