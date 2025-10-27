export function formatNumber(n: number, fractionDigits = 0) {
	const options: Intl.NumberFormatOptions = {
		maximumFractionDigits: fractionDigits,
		minimumFractionDigits: fractionDigits
	};

	// Use the user's locale (undefined) so thousands separator will match their locale.
	return new Intl.NumberFormat(undefined, options).format(n);
}

export function formatCompactNumber(num: number): string {
	if (num >= 1000000) {
		const millions = num / 1000000;
		return millions % 1 === 0 ? `${millions}m` : `${millions.toFixed(1)}m`;
	}
	if (num >= 1000) {
		const thousands = num / 1000;
		return thousands % 1 === 0 ? `${thousands}k` : `${thousands.toFixed(1)}k`;
	}
	return num.toString();
}
