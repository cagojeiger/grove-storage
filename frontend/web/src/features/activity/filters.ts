export const uuidPattern = "[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}";

export function activityFilters(route: string) {
  const supplied = new URLSearchParams(route.split("?")[1] ?? "");
  const params = new URLSearchParams();
  for (const key of ["operation_id", "account_id", "credential_id"]) {
    const value = supplied.get(key);
    if (value) params.set(key, value);
  }
  let valid = [...params.values()].every(value => new RegExp(`^${uuidPattern}$`).test(value));
  if (route.split("?")[0].endsWith("/invocations")) {
    const category = supplied.get("category");
    if (category) {
      params.set("category", category);
      valid = valid && ["management", "authentication", "all"].includes(category);
    }
  }
  return { params, valid };
}

export function activityLink(accountId: string, credentialId?: string) {
  const params = new URLSearchParams({ account_id: accountId });
  if (credentialId) params.set("credential_id", credentialId);
  return `#activity?${params}`;
}
