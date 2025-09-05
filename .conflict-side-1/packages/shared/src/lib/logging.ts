import { DEV } from 'esm-env';

export function devLog(...args: any[]) {
	if (DEV) {
		// eslint-disable-next-line no-console
		console.log(...args);
	}
}
