import { InjectionToken } from '@gitbutler/shared/context';
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
}

function isLarge(size: number | undefined) {
	return size && size > 5 * 1024 * 1024 ? true : false;
}
