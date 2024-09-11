export type UnknowObject<T> = Record<string, T>;

export function entries<T, Obj extends UnknowObject<T>>(obj: Obj): [keyof Obj, Obj[keyof Obj]][] {
	return Object.entries(obj) as [keyof Obj, Obj[keyof Obj]][];
}
