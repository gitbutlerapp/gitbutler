import type { Subscribe } from "./backend.ts";

export const handlers = {
	getUpdateStatus: () => ({ _tag: "Idle" }) as const,
	checkForUpdates: () => ({ _tag: "Unavailable" }) as const,
	downloadUpdate: () => undefined,
	installUpdate: () => undefined,
};

export const channels: Record<string, Subscribe> = {};
