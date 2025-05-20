export function pxToRem(px: number, { base = 16 }: { base?: number } = {}): string {
	return `${px / base}rem`; // Returns a string if raw is false
}
