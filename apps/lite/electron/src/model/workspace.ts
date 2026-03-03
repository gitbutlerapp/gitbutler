import { headInfoNapi } from "@gitbutler/but-sdk";

export function headInfo(projectId: string) {
	return headInfoNapi(projectId);
}
