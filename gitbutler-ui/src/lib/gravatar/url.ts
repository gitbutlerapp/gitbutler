export function gravatarUrl(id: string | undefined | null): URL | undefined {
	if (id) return new URL(`https://www.gravatar.com/avatar/${id}?s=100&r=g&d=retro`);
}
