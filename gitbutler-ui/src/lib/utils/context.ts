import { getContext as svelteGetContext } from 'svelte';

export function getContextByClass<T extends new (...args: any) => InstanceType<T>>(
	key: T
): InstanceType<T> {
	return svelteGetContext<InstanceType<T>>(key);
}
