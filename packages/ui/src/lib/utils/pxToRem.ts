export function pxToRem(px: number | undefined, base: number = 16) {
	if (px === undefined) {
		return 0;
	}

	return px / base;
}
