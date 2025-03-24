export function determinePlatform(userAgent: string) {
	const ua = userAgent.toLowerCase();

	if (ua.includes('mac os')) {
		return 'macos';
	} else if (ua.includes('windows')) {
		return 'windows';
	} else if (ua.includes('linux')) {
		return 'linux';
	}

	return 'unknown';
}
