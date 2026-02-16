import type { CellType } from '$components/commitLines/types';

const colorMap = {
	LocalOnly: 'var(--clr-commit-local)',
	LocalAndRemote: 'var(--clr-commit-remote)',
	Remote: 'var(--clr-commit-upstream)',
	Integrated: 'var(--clr-commit-integrated)',
	Error: 'var(--clr-theme-danger-element)',
	Base: 'var(--clr-commit-upstream)'
};

export function getColorFromBranchType(type: CellType | 'Error'): string {
	return colorMap[type];
}
