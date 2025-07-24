import { RemoteFile } from '$lib/files/file';
import { InjectionToken } from '@gitbutler/shared/context';
import { plainToInstance } from 'class-transformer';
import type { Tauri } from '$lib/backend/tauri';
import type { FileInfo } from '$lib/files/file';

export const FILE_SERVICE = new InjectionToken<FileService>('FileService');

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

	async listCommitFiles(projectId: string, commitId: string) {
		return plainToInstance(
			RemoteFile,
			await this.tauri.invoke<any[]>('list_commit_files', { projectId, commitId })
		).sort((a, b) => a.path?.localeCompare(b.path));
	}
}

function isLarge(size: number | undefined) {
	return size && size > 5 * 1024 * 1024 ? true : false;
}
