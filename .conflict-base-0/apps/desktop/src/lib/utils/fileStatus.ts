import { type FileStatus } from '@gitbutler/ui/components/file/types';
import type { TreeChange } from '$lib/hunks/change';

export function computeChangeStatus(change: TreeChange): FileStatus {
	switch (change.status.type) {
		case 'Addition':
			return 'A';
		case 'Deletion':
			return 'D';
		case 'Modification':
			return 'M';
		case 'Rename':
			return 'R';
	}
}
