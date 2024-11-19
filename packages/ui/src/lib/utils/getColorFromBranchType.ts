import type { CellType } from '$lib/commitLines/types';

const colorMap = {
	local: 'var(--clr-commit-local)',
	localAndRemote: 'var(--clr-commit-remote)',
	localAndShadow: 'var(--clr-commit-local)',
	remote: 'var(--clr-commit-upstream)',
	integrated: 'var(--clr-commit-integrated)',
	error: 'var(--clr-theme-err-element)'
};

export function getColorFromBranchType(type: CellType | 'error'): string {
	return colorMap[type];
}
