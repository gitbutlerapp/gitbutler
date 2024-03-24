export function pxToRem(px: number | undefined, base: number = 16) {
	if (!px) return;
	return `${px / base}rem`;
}
