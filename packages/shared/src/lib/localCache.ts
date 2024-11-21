import lscache from 'lscache';

const DEFAULT_PREFIX = 'cache';

export interface CacheOptions {
	expiry: number;
	keyPrefix: string;
}
/**
 * Helper for getting/setting values with lscache.
 */
export class LocalCache {
	expiry: number;
	keyPrefix: string | undefined;
	constructor(options: CacheOptions) {
		this.expiry = options.expiry;
		this.keyPrefix = options.keyPrefix;
	}

	get(key: string | number) {
		return lscache.get(this.transformKey(key));
	}

	set(key: string | number, value: string | number | object) {
		lscache.set(this.transformKey(key), value, this.expiry);
	}

	remove(key: string | number) {
		lscache.remove(this.transformKey(key));
	}

	private transformKey(key: string | number) {
		return DEFAULT_PREFIX + ':' + this.keyPrefix + ':' + key;
	}
}
