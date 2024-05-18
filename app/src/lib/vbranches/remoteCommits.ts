/**
 * This file should probably not be located under ../vbranches, but the reason
 * it's here is because the type is in this package.
 */
import { RemoteFile } from './types';
import { invoke } from '$lib/backend/ipc';
import { ContentSection, HunkSection, parseFileSections } from '$lib/utils/fileSections';
import { plainToInstance } from 'class-transformer';

export async function listRemoteCommitFiles(projectId: string, commitOid: string) {
	return plainToInstance(
		RemoteFile,
		await invoke<any[]>('list_remote_commit_files', { projectId, commitOid })
	).sort((a, b) => a.path?.localeCompare(b.path));
}

export function parseRemoteFiles(files: RemoteFile[]) {
	return files.map(
		(file) => [file, parseFileSections(file)] as [RemoteFile, (ContentSection | HunkSection)[]]
	);
}
