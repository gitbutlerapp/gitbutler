// We rely on the Svelte built-in concept of context, a global store for
// singletons that cascades from parent to child components. This way we
// can provide not only singletons of all services, but e.g. also
// singleton `Branch` objects within branch level components.
//
// The functions below just give us a nicer way to express the lookups.
//
// Example: `const project = getContextByClass(Project);`
import { setContext, getContext as svelteGetContext } from 'svelte';
import { writable, type Readable, type Writable } from 'svelte/store';

/**
 * Getter that returns an instance of the parameter type
 */
export function getContext<T extends new (...args: any) => InstanceType<T>>(
	key: T
): InstanceType<T> {
	const instance = svelteGetContext<InstanceType<T> | undefined>(key);
	if (!instance) throw new Error(`no instance of \`${key.name}\` in context`);
	return instance;
}

/**
 * Getter that returns an readable store of the parameter type
 */
// export function getContextStore<T extends new (...args: any) => InstanceType<T>>(
// 	key: T
// ): Readable<InstanceType<T>> {
// 	const instance = svelteGetContext<Readable<InstanceType<T>> | undefined>(key);
// 	if (!instance) throw new Error(`no instance of \`Readable<${key.name}>\` in context`);
// 	return instance;
// }

export function getContextStore<
	T extends new (...args: any) => InstanceType<T>,
	S extends Readable<InstanceType<T>> = Readable<InstanceType<T>>
>(key: T): S {
	const instance = svelteGetContext<S | undefined>(key);
	if (!instance) throw new Error(`no instance of \`Readable<${key.name}>\` in context`);
	return instance;
}

/**
 * Generic getter for store by symbol, e.g. for distinguishing local and remote commit lists.
 */
export function getContextStoreBySymbol<T, S extends Readable<T> = Readable<T>>(key: symbol): S {
	const instance = svelteGetContext<S | undefined>(key);
	if (!instance) throw new Error(`no instance of \`Readable<${key.toString}[]>\` in context`);
	return instance;
}

// Either updates or creates a store, enabling updating the value outside of
// component initialization time. Meant to be used within e.g. branches, where
// the branch id remains the same, but content mutates.
export function setContextStore<T extends new (...args: any) => InstanceType<T>>(
	key: T | symbol,
	value: InstanceType<T>
): void {
	const instance = svelteGetContext<Writable<InstanceType<T>> | undefined>(key);
	if (instance) instance.set(value);
	else setContext(key, writable(value));
}
