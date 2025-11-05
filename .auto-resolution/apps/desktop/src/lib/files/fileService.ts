import { InjectionToken } from '@gitbutler/core/context';
import type { IBackend } from '$lib/backend';
import type { FileInfo } from '$lib/files/file';
import type { ClientState } from '$lib/state/clientState.svelte';

export const FILE_SERVICE = new InjectionToken<FileService>('FileService');

export class FileService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		private backend: IBackend,
		state: ClientState
	) {
		this.api = injectEndpoints(state.backendApi);
	}

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

	findFiles(projectId: string, query: string, limit: number) {
		return this.api.endpoints.findFiles.useQuery(
			{ projectId, query, limit },
			{ forceRefetch: true }
		);
	}

	async fetchFiles(projectId: string, query: string, limit: number) {
		return await this.api.endpoints.findFiles.fetch({
			projectId,
			query,
			limit
		});
	}
}

function isLarge(size: number | undefined) {
	return size && size > 5 * 1024 * 1024 ? true : false;
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			findFiles: build.query<string[], { projectId: string; query: string; limit: number }>({
				extraOptions: { command: 'find_files' },
				query: (args) => args,
				// Keep results for a week, but use forceRefetch when using
				// the query to get eventually correct results.
				keepUnusedDataFor: 604800
			})
		})
	});
}
