export function remToPx(rem: number, opt: { base?: number } = {}) {
	const { base = 16 } = opt;
	return rem * base;
}
