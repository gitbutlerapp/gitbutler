export function toHumanReadableTime(d: Date) {
	return d.toLocaleTimeString('en-US', {
		hour: 'numeric',
		minute: 'numeric'
	});
}

export function toHumanReadableDate(d: Date) {
	return d.toLocaleDateString('en-US', {
		dateStyle: 'short',
		hour12: false
	});
}
