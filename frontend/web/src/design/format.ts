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
  return `${(value / 1024 ** unit).toLocaleString("ko-KR", { maximumFractionDigits: 1 })} ${["B", "KiB", "MiB", "GiB", "TiB"][unit]}`;
}
