import { type FileStatus } from '@gitbutler/ui/components/file/types';
import type { TreeChange } from '$lib/hunks/change';

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
