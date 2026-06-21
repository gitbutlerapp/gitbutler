import type { HowStatus } from "../../../electron/src/ipc";

export const howStatusQueryKey = ["how", "status"] as const;

export async function getHowStatus(): Promise<HowStatus> {
	return await window.how.getStatus();
}
