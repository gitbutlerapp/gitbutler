import { derived, type Readable } from 'svelte/store';

type Timeout = ReturnType<typeof setTimeout>;

export function debounce<Args extends unknown[], Return, Fn extends (...args: Args) => Return>(
	fn: Fn,
	delay: number
): (...args: Args) => void {
	let timeout: ReturnType<typeof setTimeout> | undefined;
	return (...args: Args) => {
		clearTimeout(timeout);
		timeout = setTimeout(() => fn(...args), delay);
	};
}

// Borrowed from svelte
type Stores =
	| Readable<unknown>
	| [Readable<unknown>, ...Array<Readable<unknown>>]
	| Array<Readable<unknown>>;

type StoresValues<T> =
	T extends Readable<infer U> ? U : { [K in keyof T]: T[K] extends Readable<infer U> ? U : never };

/**
 * A function similar to svelte's `derived` but debounces the calls.
 *
 * It does not support a set argument or returning cleanup functions in the
 * subject function. as it is already complex enough as is.
 */
export function debouncedDerive<S extends Stores, Return, DefaultValue>(
	targets: S,
	subject: (args: StoresValues<S>) => Return,
	defaultValue: DefaultValue,
	delay: number
): Readable<Return | DefaultValue> {
	let timeout: Timeout | undefined;

	const store = derived(
		targets,
		(derivedArgs, set) => {
			clearTimeout(timeout);
			timeout = setTimeout(() => {
				set(subject(derivedArgs));
			}, delay);

			return () => clearTimeout(timeout);
		},
		defaultValue as Return | DefaultValue
	);

	return {
		subscribe: store.subscribe
	};
}
