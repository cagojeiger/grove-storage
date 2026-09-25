import { test, expect } from "@playwright/test";
import { totals } from "../src/features/clients/model";

test("usage aggregates every storage and an empty successful report means zero", () => {
  expect(totals([], "app")).toEqual({ files: 0, bytes: 0 });
  expect(
    totals(
      [
        {
          client_id: "app",
          storage_id: "a",
          active_files: 2,
          active_bytes: 10,
        },
        {
          client_id: "other",
          storage_id: "a",
          active_files: 99,
          active_bytes: 99,
        },
        {
          client_id: "app",
          storage_id: "b",
          active_files: 3,
          active_bytes: 20,
        },
      ],
      "app",
    ),
  ).toEqual({ files: 5, bytes: 30 });
});
