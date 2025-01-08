import type { Tauri } from '$lib/backend/tauri';
import type { FileInfo } from './file';

export class FileService {
	constructor(private tauri: Tauri) {}

	async readFromWorkspace(filePath: string, projectId: string) {
		const data: FileInfo = await this.tauri.invoke('get_workspace_file', {
			relativePath: filePath,
			projectId: projectId
		});
		return {
			data,
			isLarge: isLarge(data.size)
		};
	}

	async readFromCommit(filePath: string, projectId: string, commitId: string | undefined) {
		const data: FileInfo = await this.tauri.invoke('get_commit_file', {
			relativePath: filePath,
			projectId: projectId,
			commitId
		});
		return {
			data,
			isLarge: isLarge(data.size)
		};
	}
}

function isLarge(size: number | undefined) {
	return size && size > 5 * 1024 * 1024 ? true : false;
}
