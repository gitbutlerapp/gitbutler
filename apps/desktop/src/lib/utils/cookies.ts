export function getCookie(name: string): string | undefined {
	const parsedCookies = document.cookie.split('; ').map((c) => c.split('=', 2));
	return parsedCookies.find(([k, _v]) => k === name)?.[1];
}
