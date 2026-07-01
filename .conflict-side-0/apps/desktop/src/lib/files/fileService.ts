import { InjectionToken } from "@gitbutler/core/context";
import type { IBackend } from "$lib/backend";
import type { FileInfo } from "$lib/files/file";
import type { BackendApi } from "$lib/state/backendApi";

export const FILE_SERVICE = new InjectionToken<FileService>("FileService");

export class FileService {
	constructor(
		private backend: IBackend,
		private backendApi: BackendApi,
	) {}

	async readFromWorkspace(filePath: string, projectId: string) {
		const data: FileInfo = await this.backend.invoke("get_workspace_file", {
			relativePath: filePath,
			projectId: projectId,
		});
		return {
			data,
			isLarge: isLarge(data.size),
		};
	}

	async readFromCommit(filePath: string, projectId: string, commitId: string): Promise<FileInfo> {
		return await this.backend.invoke("get_commit_file", {
			relativePath: filePath,
			projectId: projectId,
			commitId: commitId,
		});
	}

	async readFromBlob(filePath: string, projectId: string, blobId: string): Promise<FileInfo> {
		return await this.backend.invoke("get_blob_file", {
			relativePath: filePath,
			projectId: projectId,
			blobId: blobId,
		});
	}

	async readFile(path: string): Promise<Uint8Array> {
		return await this.backend.readFile(path);
	}

	async showFileInFolder(filePath: string) {
		await this.backend.invoke<void>("show_in_finder", { path: filePath });
	}

	findFiles(projectId: string, query: string, limit: number) {
		return this.backendApi.endpoints.findFiles.useQuery(
			{ projectId, query, limit },
			{ forceRefetch: true },
		);
	}

	async fetchFiles(projectId: string, query: string, limit: number) {
		return await this.backendApi.endpoints.findFiles.fetch({
			projectId,
			query,
			limit,
		});
	}
}

function isLarge(size: number | undefined) {
	return size && size > 5 * 1024 * 1024 ? true : false;
}
