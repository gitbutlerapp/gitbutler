import { invalidatesList, providesList, ReduxTag } from "$lib/state/tags";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";

export type BackupSettings = {
	backupDirectory: string;
	backupBeforeUpstreamDefault: boolean;
};

export type BackupBranch = {
	name: string;
	refName: string;
	sha: string;
};

export type BackupManifest = {
	id: string;
	createdAt: number;
	sourceProjectPath: string;
	bundlePath: string;
	size: number;
	message?: string;
	reason?: string;
	branches: BackupBranch[];
};

export type BackupRef = {
	name: string;
	sha: string;
};

export type BackupVerification = {
	valid: boolean;
	message: string;
};

export function buildBackupEndpoints(build: BackendEndpointBuilder) {
	return {
		backupSettings: build.query<BackupSettings, { projectId: string }>({
			extraOptions: { command: "get_backup_settings" },
			query: (args) => args,
			providesTags: [providesList(ReduxTag.Backups)],
		}),
		updateBackupSettings: build.mutation<
			BackupSettings,
			{ projectId: string; backupDirectory: string; backupBeforeUpstreamDefault: boolean }
		>({
			extraOptions: { command: "update_backup_settings" },
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.Backups)],
		}),
		createBackup: build.mutation<
			BackupManifest,
			{ projectId: string; branchNames: string[]; message?: string; reason?: string }
		>({
			extraOptions: { command: "create_backup", actionName: "Create Backup" },
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.Backups)],
		}),
		listBackups: build.query<BackupManifest[], { projectId: string }>({
			extraOptions: { command: "list_backups" },
			query: (args) => args,
			providesTags: [providesList(ReduxTag.Backups)],
		}),
		deleteBackup: build.mutation<void, { projectId: string; backupId: string }>({
			extraOptions: { command: "delete_backup", actionName: "Delete Backup" },
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.Backups)],
		}),
		verifyBackup: build.mutation<BackupVerification, { projectId: string; backupId: string }>({
			extraOptions: { command: "verify_backup" },
			query: (args) => args,
		}),
		listBackupRefs: build.query<BackupRef[], { projectId: string; backupId: string }>({
			extraOptions: { command: "list_backup_refs" },
			query: (args) => args,
		}),
		listBackupFiles: build.query<
			string[],
			{ projectId: string; backupId: string; refName: string }
		>({
			extraOptions: { command: "list_backup_files" },
			query: (args) => args,
		}),
		restoreBackupBranch: build.mutation<
			void,
			{
				projectId: string;
				backupId: string;
				refName: string;
				targetBranchName: string;
				overwrite: boolean;
			}
		>({
			extraOptions: { command: "restore_backup_branch", actionName: "Restore Backup Branch" },
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.BranchListing)],
		}),
		restoreBackupFiles: build.mutation<
			void,
			{ projectId: string; backupId: string; refName: string; paths: string[] }
		>({
			extraOptions: { command: "restore_backup_files", actionName: "Restore Backup Files" },
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.WorktreeChanges)],
		}),
	};
}
