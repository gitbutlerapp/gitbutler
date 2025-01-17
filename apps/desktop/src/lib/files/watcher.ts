import { RemoteFile } from './file';
import { listen, invoke } from '$lib/backend/ipc';
import { parseFileSections, type ContentSection, type HunkSection } from '$lib/utils/fileSections';
import { plainToInstance } from 'class-transformer';
import { readable, type Readable } from 'svelte/store';
import type { Project } from '$lib/project/project';

type ParsedFiles = [RemoteFile, (ContentSection | HunkSection)[]][];

export class UncommitedFilesWatcher {
	uncommitedFiles: Readable<ParsedFiles>;

	constructor(private project: Project) {
		this.uncommitedFiles = readable([] as ParsedFiles, (set) => {
			this.getUncommitedFiles().then((files) => {
				set(files);
			});

			const unsubscribe = this.listen(set);

			return unsubscribe;
		});
	}

	private async getUncommitedFiles() {
		const uncommitedFiles = await invoke<unknown[]>('get_uncommited_files', {
			id: this.project.id
		});

		const orderedFiles = plainToInstance(RemoteFile, uncommitedFiles).sort((a, b) =>
			a.path?.localeCompare(b.path)
		);

		return parseRemoteFiles(orderedFiles);
	}

	private listen(callback: (files: ParsedFiles) => void) {
		return listen<unknown[]>(`project://${this.project.id}/uncommited-files`, (event) => {
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
