/**
 * @example
 * classes("foo", undefined, "bar", "", "baz") === "foo bar baz"
 */
export const classes = (...xs: Array<string | null | undefined | false>): string =>
	// oxlint-disable-next-line typescript/strict-boolean-expressions
	xs.reduce((acc: string, x) => (x ? (acc ? `${acc} ${x}` : x) : acc), "");
