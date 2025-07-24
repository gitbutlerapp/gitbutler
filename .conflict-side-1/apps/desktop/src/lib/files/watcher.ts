import { listen, invoke } from '$lib/backend/ipc';
import { RemoteFile } from '$lib/files/file';
import { parseFileSections, type ContentSection, type HunkSection } from '$lib/utils/fileSections';
import { InjectionToken } from '@gitbutler/shared/context';
import { plainToInstance } from 'class-transformer';
import { readable } from 'svelte/store';

type ParsedFiles = [RemoteFile, (ContentSection | HunkSection)[]][];

export const UNCOMMITED_FILES_WATCHER = new InjectionToken<UncommitedFilesWatcher>(
	'UncommitedFilesWatcher'
);

export class UncommitedFilesWatcher {
	constructor() {}

	uncommittedFiles(projectId: string) {
		return readable([] as ParsedFiles, (set) => {
			this.getUncommitedFiles(projectId).then((files) => {
				set(files);
			});

			const unsubscribe = this.listen(projectId, set);

			return unsubscribe;
		});
	}

	private async getUncommitedFiles(projectId: string) {
		const uncommitedFiles = await invoke<unknown[]>('get_uncommited_files', {
			id: projectId
		});

		const orderedFiles = plainToInstance(RemoteFile, uncommitedFiles).sort((a, b) =>
			a.path?.localeCompare(b.path)
		);

		return parseRemoteFiles(orderedFiles);
	}

	private listen(projectId: string, callback: (files: ParsedFiles) => void) {
		return listen<unknown[]>(`project://${projectId}/uncommited-files`, (event) => {
			const orderedFiles = plainToInstance(RemoteFile, event.payload).sort((a, b) =>
				a.path?.localeCompare(b.path)
			);

			callback(parseRemoteFiles(orderedFiles));
		});
	}
}

function parseRemoteFiles(files: RemoteFile[]) {
	return files.map(
		(file) => [file, parseFileSections(file)] as [RemoteFile, (ContentSection | HunkSection)[]]
	);
}
