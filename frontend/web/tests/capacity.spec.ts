import { expect, test } from "@playwright/test";
import { accountedBytes, capacityInput } from "../src/features/storages/capacity";
import { capacityBytes } from "../src/features/storages/model";

test("all capacity surfaces account for stored, reserved and cleanup bytes", () => {
  expect(accountedBytes({ active_bytes: 360, reserved_bytes: 24, purge_pending_bytes: 16 })).toBe(400);
});

test("editable capacity uses readable units without losing bytes", () => {
  expect(capacityInput(1024 ** 4)).toEqual({ value: "1", unit: "TiB" });
  expect(capacityInput(1.5 * 1024 ** 3)).toEqual({ value: "1.5", unit: "GiB" });
  for (const bytes of [0, 1, 1001, 1024 ** 4, 1.5 * 1024 ** 3, 1024 ** 4 + 1, Number.MAX_SAFE_INTEGER]) {
    const result = capacityInput(bytes);
    expect(capacityBytes(result.value, result.unit)).toBe(bytes);
  }
});
