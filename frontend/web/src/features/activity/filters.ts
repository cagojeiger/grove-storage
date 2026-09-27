export const uuidPattern = "[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}";

export function activityFilters(route: string) {
  const supplied = new URLSearchParams(route.split("?")[1] ?? "");
  const params = new URLSearchParams();
  for (const key of ["account_id", "credential_id"]) {
    const value = supplied.get(key);
    if (value) params.set(key, value);
  }
  const valid = [...params.values()].every(value => new RegExp(`^${uuidPattern}$`).test(value));
  return { params, valid };
}

export function activityLink(accountId: string, credentialId?: string) {
  const params = new URLSearchParams({ account_id: accountId });
  if (credentialId) params.set("credential_id", credentialId);
  return `#activity?${params}`;
}
