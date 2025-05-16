export const componentKindConst = ['solid', 'outline', 'ghost'] as const;
export type ComponentKindType = (typeof componentKindConst)[number];

export const componentColorConst = [
	'neutral',
	'pop',
	'success',
	'error',
	'warning',
	'purple'
] as const;
export type ComponentColorType = (typeof componentColorConst)[number];
