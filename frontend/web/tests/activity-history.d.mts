import type { Page } from "@playwright/test";

export function openInvocation(page: Page, operationId: string): Promise<void>;
