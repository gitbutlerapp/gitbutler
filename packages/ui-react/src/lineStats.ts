const pluralRules = new Intl.PluralRules("en");

/**
 * A diff's `+N -N` counts in words, one phrase per side that changed, for
 * tooltips and screen readers: the green/red colouring says nothing to either.
 * Empty when nothing changed, so a caller can drop the wording along with the
 * numbers.
 */
export function describeLineStats(added: number, removed: number): Array<string> {
	const lines = (count: number) => `${count} line${pluralRules.select(count) === "one" ? "" : "s"}`;

	const parts: Array<string> = [];
	if (added > 0) parts.push(`${lines(added)} added`);
	if (removed > 0) parts.push(`${lines(removed)} removed`);
	return parts;
}
