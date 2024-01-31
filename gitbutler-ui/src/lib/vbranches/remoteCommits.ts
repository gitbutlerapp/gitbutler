/**
 * This file should probably not be located under ../vbranches, but the reason
 * it's here is because the type is in this package.
 */
import { RemoteFile } from './types';
import { parseFileSections, type ContentSection, type HunkSection } from '$lib/utils/fileSections';
import { invoke } from '@tauri-apps/api/tauri';
import { plainToInstance } from 'class-transformer';

export async function listRemoteCommitFiles(projectId: string, commitOid: string) {
	return plainToInstance(
		RemoteFile,
		await invoke<any[]>('list_remote_commit_files', { projectId, commitOid })
	)
		.map(
			(file) => [file, parseFileSections(file)] as [RemoteFile, (ContentSection | HunkSection)[]]
		)
		.sort((a, b) => a[0].path?.localeCompare(b[0].path));
}
