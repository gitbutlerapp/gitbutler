/**
 * Does nothing, but makes the type easier to read on hover.
 */
export type Prettify<T> = {
	[K in keyof T]: T[K];
} & {};
