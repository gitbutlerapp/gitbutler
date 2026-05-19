import { InjectionToken } from "@gitbutler/core/context";
import type { IBackend } from "$lib/backend";
import type { BackendApi } from "$lib/state/backendApi";
import type { BackupSettings } from "$lib/backups/backupEndpoints";

export const BACKUP_SERVICE = new InjectionToken<BackupService>("BackupService");

export class BackupService {
	constructor(
		private backendApi: BackendApi,
		private backend: IBackend,
	) {}

	settings(projectId: string) {
		return this.backendApi.endpoints.backupSettings.useQuery({ projectId });
	}

	async getSettings(projectId: string) {
		return await this.backendApi.endpoints.backupSettings.fetch({ projectId });
	}

	backups(projectId: string) {
		return this.backendApi.endpoints.listBackups.useQuery({ projectId });
	}

	refs(projectId: string, backupId: string | undefined) {
		return this.backendApi.endpoints.listBackupRefs.useQuery({ projectId, backupId: backupId ?? "" });
	}

	files(projectId: string, backupId: string | undefined, refName: string | undefined) {
		return this.backendApi.endpoints.listBackupFiles.useQuery(
			{ projectId, backupId: backupId ?? "", refName: refName ?? "" },
		);
	}

	async createBackup(args: {
		projectId: string;
		branchNames: string[];
		message?: string;
		reason?: string;
	}) {
		return await this.backendApi.endpoints.createBackup.mutate(args);
	}

	async deleteBackup(projectId: string, backupId: string) {
		await this.backendApi.endpoints.deleteBackup.mutate({ projectId, backupId });
	}

	async verifyBackup(projectId: string, backupId: string) {
		return await this.backendApi.endpoints.verifyBackup.mutate({ projectId, backupId });
	}

	async restoreBranch(args: {
		projectId: string;
		backupId: string;
		refName: string;
		targetBranchName: string;
		overwrite: boolean;
	}) {
		await this.backendApi.endpoints.restoreBackupBranch.mutate(args);
	}

	async restoreFiles(args: {
		projectId: string;
		backupId: string;
		refName: string;
		paths: string[];
	}) {
		await this.backendApi.endpoints.restoreBackupFiles.mutate(args);
	}

	async updateSettings(projectId: string, settings: BackupSettings) {
		return await this.backendApi.endpoints.updateBackupSettings.mutate({
			projectId,
			...settings,
		});
	}

	async chooseBackupDirectory(defaultPath?: string) {
		return await this.backend.filePicker({
			title: "Choose backup folder",
			directory: true,
			defaultPath,
			canCreateDirectories: true,
		});
	}
}
