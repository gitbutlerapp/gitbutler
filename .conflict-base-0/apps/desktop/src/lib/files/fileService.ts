import { InjectionToken } from '@gitbutler/shared/context';
import type { IBackend } from '$lib/backend';
import type { FileInfo } from '$lib/files/file';

export const FILE_SERVICE = new InjectionToken<FileService>('FileService');

export class FileService {
	constructor(private backend: IBackend) {}

	async readFromWorkspace(filePath: string, projectId: string) {
		const data: FileInfo = await this.backend.invoke('get_workspace_file', {
			relativePath: filePath,
			projectId: projectId
		});
		return {
			data,
			isLarge: isLarge(data.size)
		};
	}

	async readFile(path: string): Promise<Uint8Array> {
		return await this.backend.readFile(path);
	}

	async showFileInFolder(filePath: string) {
		await this.backend.invoke<void>('show_in_finder', { path: filePath });
	}
}

function isLarge(size: number | undefined) {
	return size && size > 5 * 1024 * 1024 ? true : false;
}
