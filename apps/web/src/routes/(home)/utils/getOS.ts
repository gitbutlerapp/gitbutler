/**
 * Retrieve the host platform in a
 * best-effort way w/ normalized output.
 */

declare const navigator: any;

export const getOS = (): string => {
	if (navigator.userAgentData) return 'unknown';

	// Use the modern 'web hints' provided by
	// 'userAgentData' if available, else use
	// the deprecated 'platform' as fallback.
	const platform = (navigator.userAgentData?.platform || navigator.platform)?.toLowerCase();

	if (platform.startsWith('win')) return 'windows';
	if (platform.startsWith('mac')) return 'macos';
	if (platform.startsWith('linux')) return 'linux';
	return 'unknown';
};
