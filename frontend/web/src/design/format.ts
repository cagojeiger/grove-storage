export const time = (value: string) => new Date(value).toLocaleString("en-US");

export function duration(milliseconds: number) {
  return milliseconds < 1000
    ? `${milliseconds.toLocaleString("en-US")} ms`
    : `${(milliseconds / 1000).toLocaleString("en-US", { maximumFractionDigits: 1 })} s`;
}

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
  return `${(value / 1024 ** unit).toLocaleString("en-US", { maximumFractionDigits: 1 })} ${["B", "KiB", "MiB", "GiB", "TiB"][unit]}`;
}
