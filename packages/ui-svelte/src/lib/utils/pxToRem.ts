export function pxToRem(px: number | undefined, zoom = 1.0) {
	if (px === undefined) {
		return 0;
	}
	return px / (zoom * 16);
}

export function remToPx(rem: number | undefined, zoom = 1.0) {
	if (rem === undefined) {
		return 0;
	}
	return 16 * rem * zoom;
}
