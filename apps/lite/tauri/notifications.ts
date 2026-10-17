import type { Subscribe } from "./backend.ts";

export const handlers = {
	showNotification: () => undefined,
};

export const channels: Record<string, Subscribe> = {};
