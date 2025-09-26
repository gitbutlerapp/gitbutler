/**
 * Parameter matcher for owner slugs.
 * Matches URL-safe strings that can be used as owner identifiers.
 * Allows letters, numbers, hyphens, underscores, and dots.
 */
export function match(param: string): param is string {
	return /^[a-zA-Z0-9_.-]+$/.test(param);
}
