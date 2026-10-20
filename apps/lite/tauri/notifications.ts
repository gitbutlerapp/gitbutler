import type { ShowNotificationParams } from "#electron/ipc.ts";
import { backend, listenNow, type Subscribe } from "./backend.ts";

export const handlers = {
	showNotification: async ({ id, title, body }: ShowNotificationParams): Promise<void> => {
		await backend("show_notification", { id, title, body });
	},
};

export const channels: Record<string, Subscribe> = {
	notificationClick: (listener) => listenNow("notification-click", listener),
};
