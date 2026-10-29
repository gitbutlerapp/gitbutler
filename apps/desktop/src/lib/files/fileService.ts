import { checkoutOf } from "$lib/selection/key";
import { InjectionToken } from "@gitbutler/core/context";
import type { IBackend } from "$lib/backend";
import type { BackendApi } from "$lib/state/backendApi";
import type { FileInfo } from "@gitbutler/but-sdk";

export const FILE_SERVICE = new InjectionToken<FileService>("FileService");

export class FileService {
	constructor(
		private backend: IBackend,
		private backendApi: BackendApi,
	) {}

	async readFromWorkspace(filePath: string, projectId: string): Promise<FileInfo> {
		return await this.backend.invoke("get_workspace_file", {
			relativePath: filePath,
			projectId: projectId,
		});
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

	/** The absolute path of `path`, relative to the linked `worktree` or the main checkout. */
	async workdirPath(projectId: string, worktree: string | undefined, path: string) {
		return await this.backendApi.endpoints.workdirPath.fetch({ projectId, worktree, path });
	}

	/** What revealing in the file manager is called on this platform. */
	get showInFolderLabel(): string {
		switch (this.backend.platformName) {
			case "macos":
				return "Show in Finder";
			case "windows":
				return "Show in Explorer";
			default:
				return "Show in File Manager";
		}
	}

	/**
	 * Reveals `path`, relative to the linked `worktree` or the main checkout, in
	 * the file manager; an empty `path` reveals the checkout.
	 */
	async showFileInFolder(projectId: string, worktree: string | undefined, path: string) {
		const checkout = checkoutOf({ worktree });
		await this.backend.invoke<void>("show_in_finder", { projectId, checkout, path });
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
