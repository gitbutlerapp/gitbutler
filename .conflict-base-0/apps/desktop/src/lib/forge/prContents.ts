import { getEphemeralStorageItem, setEphemeralStorageItem } from '@gitbutler/shared/persisted';
import { type Subscriber, type Unsubscriber } from 'svelte/store';
import type { Commit } from '$lib/branches/v3';

/**
 * A custom persisted store that makes it easier to manage pr descriptions.
 *
 * This store combines persistence with an ability to override the value
 * dispatched to subscribes. In practice this means we automatically
 * suggest a pr title based on commit data, but persist what the user
 * manually enters.
 */
export class PrPersistedStore {
	private persisted = '';

	_default = '';
	private subscribers: Subscriber<string>[] = [];

	constructor(
		private args: {
			cacheKey: string;
			commits: Commit[];
			defaultFn: (commits: Commit[]) => string;
		}
	) {
		this.persisted = (getEphemeralStorageItem(this.args.cacheKey) || '') as string;
	}

	subscribe(callback: Subscriber<string>): Unsubscriber {
		this.subscribers.push(callback);
		this.dispatch(this.persisted);

		return () => {
			this.subscribers = this.subscribers.filter((cb) => cb !== callback);
		};
	}

	dispatchCurrent() {
		this.dispatch(this.persisted !== '' ? this.persisted : this._default);
	}

	dispatch(value: string) {
		const dispatchValue = isEmptyOrUndefined(value) ? this._default : value;
		for (const subscriber of this.subscribers) {
			subscriber(dispatchValue);
		}
	}

	set(value: string) {
		const storedValue = value === this._default ? '' : value;
		setEphemeralStorageItem(this.args.cacheKey, storedValue, 1440);
		this.persisted = value;
		this.dispatch(storedValue);
	}

	append(value: string) {
		this.set(this.persisted + value);
	}

	reset() {
		this.set('');
	}

	setDefault(commits: Commit[]) {
		if (commits.length === 1) {
			this._default = this.args.defaultFn(commits);
			this.dispatchCurrent();
		}
	}

	get default() {
		return this._default;
	}
}

function isEmptyOrUndefined(line?: string) {
	return line === '\n' || line === '' || line === undefined;
}
