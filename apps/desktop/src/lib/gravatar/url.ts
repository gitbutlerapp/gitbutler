export function gravatarUrl(id: string | undefined | null): string | undefined {
	if (id) return `https://www.gravatar.com/avatar/${id}?s=100&r=g&d=retro`;
}
