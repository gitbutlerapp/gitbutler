export function formatNumber(n: number, fractionDigits = 0) {
	const options: Intl.NumberFormatOptions = {
		maximumFractionDigits: fractionDigits,
		minimumFractionDigits: fractionDigits
	};

	// Use the user's locale (undefined) so thousands separator will match their locale.
	return new Intl.NumberFormat(undefined, options).format(n);
}
