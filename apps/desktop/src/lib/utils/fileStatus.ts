import { RemoteFile, type AnyFile } from '$lib/vbranches/types';

export type FileStatus = 'A' | 'M' | 'D';

export function computeFileStatus(file: AnyFile): FileStatus {
	if (file instanceof RemoteFile) {
		if (file.hunks.length === 1) {
			const diff = file.hunks[0].diff;
			if (/^@@ -1,1 /.test(diff)) return 'A';
			if (/^@@ -0,0 /.test(diff)) return 'A';
			if (/^@@ -\d+,\d+ \+0,0/.test(diff)) return 'D';
			if (/^@@ -\d+,\d+ \+1,1/.test(diff)) return 'D';
		}
		return 'M';
	}

	if (file.hunks.length === 1) {
		const changeType = file.hunks[0].changeType;
		if (changeType === 'added') {
			return 'A';
		} else if (changeType === 'deleted') {
			return 'D';
		}
	}
	return 'M';
}
