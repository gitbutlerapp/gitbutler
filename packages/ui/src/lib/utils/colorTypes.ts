export const componentKindConst = ['solid', 'outline', 'ghost'] as const;
export type ComponentKindType = (typeof componentKindConst)[number];

export const componentColorConst = ['gray', 'pop', 'safe', 'danger', 'warning', 'purple'] as const;
export type ComponentColorType = (typeof componentColorConst)[number];
