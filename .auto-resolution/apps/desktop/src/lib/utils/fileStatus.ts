import { type AnyFile } from '$lib/files/file';
import { type FileStatus } from '@gitbutler/ui/components/file/types';
import type { TreeChange } from '$lib/hunks/change';

export function computeFileStatus(file: AnyFile): FileStatus {
	if (file.hunks.length === 1) {
		const changeType = file.hunks[0]?.changeType;
		if (changeType === 'added' || changeType === 'untracked') {
			return 'A';
		} else if (changeType === 'deleted') {
			return 'D';
		}
	}
	return 'M';
}

export function computeChangeStatus(change: TreeChange): FileStatus {
	switch (change.status.type) {
		case 'Addition':
			return 'addition';
		case 'Deletion':
			return 'deletion';
		case 'Modification':
			return 'modification';
		case 'Rename':
			return 'rename';
	}
}
