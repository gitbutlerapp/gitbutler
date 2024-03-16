import { getContext as svelteGetContext } from 'svelte';
import type { Readable } from 'svelte/store';

export function getContextByClass<T extends new (...args: any) => InstanceType<T>>(
	key: T
): InstanceType<T> {
	return svelteGetContext<InstanceType<T>>(key);
}

export function getContextStoreByClass<T extends new (...args: any) => InstanceType<T>>(
	key: T
): Readable<InstanceType<T>> {
	return svelteGetContext<Readable<InstanceType<T>>>(key);
}
