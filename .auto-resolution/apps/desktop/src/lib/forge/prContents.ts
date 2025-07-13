import {
	getEphemeralStorageItem,
	persisted,
	setEphemeralStorageItem,
	type Persisted
} from '@gitbutler/shared/persisted';
import { type Subscriber, type Unsubscriber } from 'svelte/store';
import type { Commit } from '$lib/branches/v3';
import type { TemplateService } from '$lib/forge/templateService';

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
			defaultFn: (commits: Commit[]) => Promise<string>;
		}
	) {
		this.persisted = (getEphemeralStorageItem(this.args.cacheKey) || '') as string;
	}

	subscribe(callback: Subscriber<string>): Unsubscriber {
		this.subscribers.push(callback);
		this.dispatch(!isEmptyOrUndefined(this.persisted) ? this.persisted : this._default);

		return () => {
			this.subscribers = this.subscribers.filter((cb) => cb !== callback);
		};
	}

	dispatchCurrent() {
		this.dispatch(!isEmptyOrUndefined(this.persisted) ? this.persisted : this._default);
	}

	dispatch(value: string) {
		for (const subscriber of this.subscribers) {
			subscriber(value);
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

	async setDefault(commits: Commit[]) {
		this._default = await this.args.defaultFn(commits);
		this.dispatchCurrent();
	}

	get default() {
		return this._default;
	}
}

function isEmptyOrUndefined(line?: string) {
	return line === '\n' || line === '' || line === undefined;
}

export class PrTemplateStore {
	readonly templatePath: Persisted<string | undefined>;
	readonly templateEnabled: Persisted<boolean>;

	constructor(
		private projectId: string,
		private forgeName: string,
		private templateService: TemplateService
	) {
		this.templatePath = persisted<string | undefined>(undefined, `last-template-${projectId}`);
		this.templateEnabled = persisted(false, `enable-template-${projectId}`);
	}

	async getAvailable(): Promise<string[]> {
		return await this.templateService.getAvailable(this.forgeName);
	}

	async getTemplateContent(templatePath: string): Promise<string> {
		return await this.templateService.getContent(this.forgeName, templatePath);
	}
}
