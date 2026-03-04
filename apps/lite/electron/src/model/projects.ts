import { listProjectsNapi } from "@gitbutler/but-sdk";

export async function listProjects() {
	return await listProjectsNapi([]);
}
