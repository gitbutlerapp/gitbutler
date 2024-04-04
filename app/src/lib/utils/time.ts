export function toHumanReadableTime(timestamp: number) {
	return new Date(timestamp).toLocaleTimeString('en-US', {
		hour: 'numeric',
		minute: 'numeric'
	});
}

export function toHumanReadableDate(timestamp: number) {
	return new Date(timestamp).toLocaleDateString('en-US', {
		dateStyle: 'short'
	});
}
