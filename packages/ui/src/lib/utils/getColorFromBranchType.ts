import type { CellType } from '$lib/commitLines/types';

const colorMap = {
	local: 'var(--clr-commit-local)',
	localAndRemote: 'var(--clr-commit-remote)',
	localAndShadow: 'var(--clr-commit-local)',
	remote: 'var(--clr-commit-upstream)',
	integrated: 'var(--clr-commit-integrated)'
};

export function getColorFromBranchType(type: CellType): string {
	return colorMap[type];
}
