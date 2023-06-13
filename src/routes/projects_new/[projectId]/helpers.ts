import type { Commit, File } from './types';

export function createCommit(files: File[], isShadow: boolean): Commit {
	return {
		id: `commit-${Date.now()}`,
		description: '',
		kind: 'commit',
		files: files,
		isDndShadowItem: isShadow
	};
}
