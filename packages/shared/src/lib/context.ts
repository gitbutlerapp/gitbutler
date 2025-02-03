// We rely on the Svelte built-in concept of context, a global store for
// singletons that cascades from parent to child components. This way we
// can provide not only singletons of all services, but e.g. also
// singleton `Branch` objects within branch level components.
//
// The functions below just give us a nicer way to express the lookups.

import { setContext, getContext as svelteGetContext } from 'svelte';
import { writable, type Readable, type Writable } from 'svelte/store';

type Class = new (...args: any) => any;

/**
 * Getter that returns an instance of the parameter type
 */
export function getContext<T extends Class>(key: T): InstanceType<T> {
	const instance = svelteGetContext<InstanceType<T> | undefined>(key);
	if (!instance) throw new Error(`no instance of \`${key.name}\` in context`);
	return instance;
}

/**
 * Optional getter that returns an instance of the parameter type
 */
export function maybeGetContext<T extends Class>(key: T): InstanceType<T> | undefined {
	return svelteGetContext<InstanceType<T> | undefined>(key);
}

/**
 * Getter that returns an readable store of the parameter type
 */
export function getContextStore<
	T extends Class,
	S extends Readable<InstanceType<T>> = Readable<InstanceType<T>>
>(key: T): S {
	const instance = svelteGetContext<S | undefined>(key);
	if (!instance) throw new Error(`no instance of \`Readable<${key.name}>\` in context`);
	return instance;
}

export function maybeGetContextStore<
	T extends Class,
	S extends Readable<InstanceType<T>> = Readable<InstanceType<T>>
>(key: T): S | undefined {
	return svelteGetContext<S | undefined>(key);
}

/**
 * Either updates or creates a store, enabling updating the value outside of
 * component initialization time. Meant to be used within e.g. branches, where
 * the branch id remains the same, but content mutates.
 */
export function createContextStore<T extends Class>(
	key: T | symbol,
	value: InstanceType<T> | undefined
): Writable<InstanceType<T> | undefined> {
	const instance = svelteGetContext<Writable<InstanceType<T>> | undefined>(key);
	if (instance) {
		throw new Error('Context store already defined for key: ' + key.toString());
	}
	const store = writable(value);
	setContext(key, store);
	return store;
}

/**
 * When using dependency injection for things that are not unique by type you often
 * turn to an injection token, such as a `Symbol()` that you can use with `getContext`.
 *
 * Instead of referencing the same injection token, we use this function to create
 * a pair of getter and setter functions. These can be named and exported as shown
 * in the example below.
 *
 * Example:
 * ```
 *   export const [getSpecialCommits, setSpecialCommits] = buildContextStore<Commit[]>();`
 * ```
 */
export function buildContextStore<T, S extends Readable<T> = Readable<T>>(
	name: string
): [() => S, (value: T | undefined) => Writable<T>] {
	const identifier = Symbol(name);
	return [
		() => {
			return getContextStoreBySymbol<T, S>(identifier);
		},
		(value: T | undefined) => {
			return createContextStore(identifier, value);
		}
	];
}

/**
 * Generic getter for store by symbol, e.g. for distinguishing local and remote commit lists.
 *
 * TODO: Make `UserSettings` a class rather than interface so we don't need this exported.
 */
export function getContextStoreBySymbol<T, S extends Readable<T> = Readable<T>>(key: symbol): S {
	const instance = svelteGetContext<S | undefined>(key);
	if (!instance) throw new Error(`no instance of \`Readable<${key.toString()}[]>\` in context`);
	return instance;
}

export function buildContext<T>(name: string): [() => T, (value: T | undefined) => void] {
	const identifier = Symbol(name);
	return [
		() => {
			return svelteGetContext<T>(identifier);
		},
		(value: T | undefined) => {
			setContext(identifier, value);
		}
	];
}

type Constructor<T> = new (...args: any[]) => T;

/**
 * Inject multiple dependencies using positional rest parameters.
 *
 * Example: ```
 *  const [serviceA, serviceB] = inject(ServiceA, ServiceB);
 * ```
 */
export function inject<T extends Constructor<any>[]>(
	...constructors: T
): { [K in keyof T]: InstanceType<T[K]> } {
	return constructors.map((Ctor) => getContext<InstanceType<typeof Ctor>>(Ctor)) as {
		[K in keyof T]: InstanceType<T[K]>;
	};
}
