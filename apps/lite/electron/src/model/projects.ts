import { listProjectsNapi } from '@gitbutler/but-sdk';

export function listProjects() {
	return listProjectsNapi([]);
}
