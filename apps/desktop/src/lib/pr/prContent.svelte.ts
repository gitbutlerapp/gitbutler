import { getEphemeralStorageItem, setEphemeralStorageItem } from '@gitbutler/shared/persisted';
import type { DetailedCommit } from '$lib/vbranches/types';

const PERSITANCE_TIME_MIN = 5;

function getPersistedBodyKey(projectId: string, branchName: string) {
	return 'seriesCurrentPRBody_' + projectId + '_' + branchName;
}

function getPersistedTitleKey(projectId: string, branchName: string) {
	return 'seriesCurrentPRTitle_' + projectId + '_' + branchName;
}

export function setPersistedPRBody(projectId: string, branchName: string, body: string): void {
	const key = getPersistedBodyKey(projectId, branchName);
	setEphemeralStorageItem(key, body, PERSITANCE_TIME_MIN);
}

export function getPersistedPRBody(projectId: string, branchName: string): string | undefined {
	const key = getPersistedBodyKey(projectId, branchName);
	const content = getEphemeralStorageItem(key);

	if (typeof content === 'string') {
		return content;
	}

	return undefined;
}

export function setPersistedPRTitle(projectId: string, branchName: string, title: string): void {
	const key = getPersistedTitleKey(projectId, branchName);
	setEphemeralStorageItem(key, title, PERSITANCE_TIME_MIN);
}

export function getPersistedPRTitle(projectId: string, branchName: string): string | undefined {
	const key = getPersistedTitleKey(projectId, branchName);
	const content = getEphemeralStorageItem(key);

	if (typeof content === 'string') {
		return content;
	}

	return undefined;
}

export class ReactivePRTitle {
	private _value = $state<string>('');

	constructor(
		private projectId: string,
		private isDisplay: boolean,
		private existingTitle: string | undefined,
		private commits: DetailedCommit[],
		private branchName: string
	) {
		const persistedTitle = getPersistedPRTitle(projectId, branchName);
		this._value = persistedTitle ?? this.getDefaultTitle();
	}

	private getDefaultTitle(): string {
		if (this.isDisplay) return this.existingTitle ?? '';
		// In case of a single commit, use the commit summary for the title
		if (this.commits.length === 1) {
			const commit = this.commits[0];
			return commit?.descriptionTitle ?? '';
		}
		return this.branchName;
	}

	get value() {
		return this._value;
	}

	set(value: string) {
		this._value = value;
		setPersistedPRTitle(this.projectId, this.branchName, value);
	}
}

export class ReactivePRBody {
	private _value = $state<string>('');

	constructor(
		private projectId: string,
		private isDisplay: boolean,
		private branchDescription: string | undefined,
		private existingBody: string | undefined,
		private commits: DetailedCommit[],
		private templateBody: string | undefined,
		private branchName: string
	) {
		const persistedBody = getPersistedPRBody(projectId, branchName);
		this._value = persistedBody ?? this.getDefaultBody();
	}

	getDefaultBody(): string {
		if (this.isDisplay) return this.existingBody ?? '';
		if (this.branchDescription) return this.branchDescription;
		if (this.templateBody) return this.templateBody;
		// In case of a single commit, use the commit description for the body
		if (this.commits.length === 1) {
			const commit = this.commits[0];
			return commit?.descriptionBody ?? '';
		}
		return '';
	}

	get value() {
		return this._value;
	}

	set(value: string) {
		this._value = value;
		setPersistedPRBody(this.projectId, this.branchName, value);
	}

	append(value: string) {
		this.set(this._value + value);
	}

	reset() {
		this.set('');
	}
}
