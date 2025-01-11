export const componentKindConst = ['solid', 'soft', 'outline', 'ghost'] as const;
export type ComponentKindType = (typeof componentKindConst)[number];

export const componentColorConst = [
	'neutral',
	'ghost',
	'pop',
	'success',
	'error',
	'warning',
	'purple'
];
export type ComponentColorType = (typeof componentColorConst)[number];
