export function remToPx(
	rem: number,
	opt: { base?: number; asNumber?: boolean } = { asNumber: true }
) {
	const { base = 16, asNumber = false } = opt;
	return asNumber ? rem * base : `${rem * base}px`;
}
