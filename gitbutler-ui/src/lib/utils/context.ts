import { getContext as svelteGetContext } from 'svelte';
import type { Readable } from 'svelte/store';

export function getContextByClass<T extends new (...args: any) => InstanceType<T>>(
	key: T
): InstanceType<T> {
	const instance = svelteGetContext<InstanceType<T> | undefined>(key);
	if (!instance) throw new Error(`no instance of \`${key.name}\` in context`);
	return instance;
}

export function getContextStoreByClass<T extends new (...args: any) => InstanceType<T>>(
	key: T
): Readable<InstanceType<T>> {
	const instance = svelteGetContext<Readable<InstanceType<T>> | undefined>(key);
	if (!instance) throw new Error(`no instance of \`Readable<${key.name}>\` in context`);
	return instance;
}
