import { headInfoNapi } from "@gitbutler/but-sdk";

export async function headInfo(projectId: string) {
	return await headInfoNapi(projectId);
}
