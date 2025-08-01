export function typedKeys<T extends Record<string, unknown>>(obj: T): (keyof T)[] {
	return Object.keys(obj) as (keyof T)[];
}
