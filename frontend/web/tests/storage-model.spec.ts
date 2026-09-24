import { expect, test } from "@playwright/test";
import {
  capacityBytes,
  idPattern,
  storageSpec,
  mutationMessage,
} from "../src/features/storages/model";
import { ApiError } from "../src/api/http";

test("storage ID pattern matches the DB slug with HTML unicode-set semantics", () => {
  const pattern = new RegExp(`^(?:${idPattern})$`, "v");
  for (const id of ["a", "0", "home-archive", "a".repeat(64)])
    expect(pattern.test(id)).toBe(true);
  for (const id of ["", "-a", "a-", "UPPER", "a/b", "a".repeat(65)])
    expect(pattern.test(id)).toBe(false);
});

test("capacity conversion is exact and rejects unsafe integers", () => {
  expect(capacityBytes("1.5", "GiB")).toBe(1610612736);
  expect(capacityBytes("0", "B")).toBe(0);
  expect(capacityBytes(String(Number.MAX_SAFE_INTEGER), "B")).toBe(
    Number.MAX_SAFE_INTEGER,
  );
  for (const value of [
    "",
    "-1",
    "NaN",
    "Infinity",
    "1e3",
    "0.1",
    "9007199254740992",
    "9007199254740990.1",
    "1.00000000000000000001",
  ])
    expect(() => capacityBytes(value, "B")).toThrow();
  expect(() => capacityBytes("999999999", "TiB")).toThrow();
});

test("fs payload excludes every S3 field including credentials", () => {
  const data = new FormData();
  for (const [key, value] of Object.entries({
    root_path: "/data",
    capacity: "42",
    unit: "B",
    secret_key: "secret",
    endpoint: "https://s3.test",
    force_relay: "on",
  }))
    data.set(key, value);
  expect(storageSpec(data, "fs")).toEqual({
    kind: "fs",
    root_path: "/data",
    capacity_bytes: 42,
  });
});

test("S3 optional public endpoint is omitted and URL credentials are rejected", () => {
  const data = new FormData();
  for (const [key, value] of Object.entries({
    endpoint: "https://s3.test",
    capacity: "0",
    unit: "B",
    secret_key: " secret ",
  }))
    data.set(key, value);
  expect(storageSpec(data, "s3")).not.toHaveProperty("public_endpoint");
  expect(storageSpec(data, "s3")).toHaveProperty("secret_key", " secret ");
  for (const endpoint of [
    "ftp://s3.test",
    "http://user:secret@s3.test",
    "invalid",
  ]) {
    data.set("endpoint", endpoint);
    expect(() => storageSpec(data, "s3")).toThrow();
  }
});

test("mutation errors separate conflicts from unknown outcomes", () => {
  expect(mutationMessage(new ApiError(409), "delete")).toContain(
    "클라이언트 또는 파일",
  );
  expect(mutationMessage(new ApiError(409), "replace")).toContain("주소");
  expect(mutationMessage(new ApiError(409), "create")).toContain("이미 등록");
  expect(mutationMessage(new ApiError(500), "create")).toContain("변경 결과");
  expect(mutationMessage(new Error("secret"), "create")).not.toContain(
    "secret",
  );
});
