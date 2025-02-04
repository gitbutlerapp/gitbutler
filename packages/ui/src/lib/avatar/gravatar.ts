export function gravatarUrl(id: string | undefined | null): string | undefined {
	if (id) return `https://www.gravatar.com/avatar/${id}?s=100&r=g&d=retro`;
}

export async function gravatarUrlFromEmail(email: string): Promise<string> {
	const encoder = new TextEncoder();
	const strippedEmail = (email || '').toLocaleLowerCase().replaceAll(' ', '');
	const data = encoder.encode(strippedEmail);
	const hash = await crypto.subtle.digest('SHA-256', data);
	const hashArray = Array.from(new Uint8Array(hash));
	const hashHex = hashArray.map((byte) => byte.toString(16).padStart(2, '0')).join('');

	return gravatarUrl(hashHex) as string;
}
