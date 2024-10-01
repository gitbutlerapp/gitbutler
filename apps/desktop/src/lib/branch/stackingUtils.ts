export type BranchColor = 'neutral' | 'integrated';

const colorMap = {
	neutral: 'var(--clr-scale-ntrl-80)',
	integrated: 'var(--clr-commit-integrated)'
};

export function getColorFromBranchType(type: BranchColor) {
	return colorMap[type];
}
