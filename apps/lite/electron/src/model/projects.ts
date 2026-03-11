import { listProjectsStatelessNapi } from "@gitbutler/but-sdk";

export async function listProjects() {
	return await listProjectsStatelessNapi();
}
