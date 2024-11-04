import type { CellType } from '@gitbutler/ui/commitLines/types';

const colorMap = {
	local: 'var(--clr-commit-local)',
	localAndRemote: 'var(--clr-commit-remote)',
	localAndShadow: 'var(--clr-commit-local)',
	remote: 'var(--clr-commit-remote)',
	integrated: 'var(--clr-commit-integrated)'
};

export function getColorFromBranchType(type: CellType): string {
	return colorMap[type];
}
