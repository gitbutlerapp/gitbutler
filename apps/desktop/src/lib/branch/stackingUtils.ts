const colorMap = {
	local: 'var(--clr-commit-local)',
	localAndRemote: 'var(--clr-commit-remote)',
	localAndShadow: 'var(--clr-commit-local)',
	remote: 'var(--clr-commit-remote)',
	integrated: 'var(--clr-commit-integrated)'
};

export function getColorFromBranchType(type: keyof typeof colorMap): string {
	return colorMap[type];
}
