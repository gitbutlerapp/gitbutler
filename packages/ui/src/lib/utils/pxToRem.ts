export function pxToRem(px: number | undefined) {
	if (px === undefined) {
		return 0;
	}
	return px / 16;
}

export function remToPx(rem: number | undefined) {
	if (rem === undefined) {
		return 0;
	}
	return 16 * rem;
}
