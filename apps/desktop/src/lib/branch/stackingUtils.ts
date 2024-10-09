import type { CommitStatus } from '$lib/vbranches/types';

const colorMap = {
	local: 'var(--clr-commit-local)',
	localAndRemote: 'var(--clr-commit-remote)',
	remote: 'var(--clr-commit-remote)',
	integrated: 'var(--clr-commit-integrated)'
};

export function getColorFromBranchType(type: CommitStatus) {
	return colorMap[type];
}
