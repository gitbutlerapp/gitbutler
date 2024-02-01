import { RemoteFile, type File } from '$lib/vbranches/types';

export type FileStatus = 'A' | 'M' | 'D';

export function computeFileStatus(file: File | RemoteFile): FileStatus {
	if (file instanceof RemoteFile) {
		// TODO: How do we compute this for remote files?
		return 'M';
	}
	if (file.hunks.length == 1) {
		const changeType = file.hunks[0].changeType;
		if (changeType == 'added') {
			return 'A';
		} else if (changeType == 'deleted') {
			return 'D';
		}
	}
	return 'M';
}
