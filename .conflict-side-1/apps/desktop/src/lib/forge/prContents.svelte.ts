import MessageEditor from '$components/v3/editor/MessageEditor.svelte';
import { splitMessage } from '$lib/utils/commitMessage';
import { getEphemeralStorageItem, setEphemeralStorageItem } from '@gitbutler/shared/persisted';
import type { Commit } from '$lib/branches/v3';

const PERSITANCE_TIME_MIN = 5;

function getPersistedBodyKey(projectId: string, branchName: string) {
	return 'seriesCurrentPRBody_' + projectId + '_' + branchName;
}

function getPersistedTitleKey(projectId: string, branchName: string) {
	return 'seriesCurrentPRTitle_' + projectId + '_' + branchName;
}

export function setPersistedPRBody(
	projectId: string,
	branchName: string,
	body: string | undefined
): void {
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

export function setPersistedPRTitle(
	projectId: string,
	branchName: string,
	title: string | undefined
): void {
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
		private commits: Commit[],
		private branchName: string
	) {
		const persistedTitle = getPersistedPRTitle(projectId, branchName);
		this._value = persistedTitle ?? this.getDefaultTitle();
	}

	private getDefaultTitle(): string {
		// In case of a single commit, use the commit summary for the title
		if (this.commits.length === 1) {
			const commit = this.commits[0]!;
			return splitMessage(commit.message).title;
		}
		return this.branchName;
	}

	get value() {
		return this._value;
	}

	set(value: string | undefined) {
		this._value = value ?? '';

		// Don't persist the default value
		if (value !== this.getDefaultTitle()) {
			setPersistedPRTitle(this.projectId, this.branchName, value);
		}
	}

	reset() {
		this.set(undefined);
	}
}

function isEmptyLine(line: string) {
	return line === '\n' || line === '';
}

export class ReactivePRBody {
	private _value = $state<string>('');
	private projectId: string | undefined;
	private branchDescription: string | undefined;
	private commits: Commit[] | undefined;
	private _templateBody = $state<string | undefined>(undefined);
	private branchName: string | undefined;
	private _descriptionInput = $state<ReturnType<typeof MessageEditor>>();

	init(
		projectId: string,
		branchDescription: string | undefined,
		commits: Commit[],
		branchName: string
	) {
		this.projectId = projectId;
		this.branchDescription = branchDescription;
		this.commits = commits;
		this.branchName = branchName;

		const persistedBody = getPersistedPRBody(projectId, branchName);
		const value =
			persistedBody === undefined || isEmptyLine(persistedBody)
				? this.getDefaultBody()
				: persistedBody;
		this._value = value;

		this._descriptionInput?.setText(value);
	}

	getDefaultBody(): string {
		if (this.branchDescription) return this.branchDescription;
		if (this._templateBody) return this._templateBody;
		// In case of a single commit, use the commit description for the body
		const commits = this.commits ?? [];
		if (commits.length === 1) {
			const commit = commits[0]!;
			return splitMessage(commit.message).description;
		}
		return '';
	}

	get value() {
		return this._value;
	}

	/**
	 * Set the value of the PR body.
	 *
	 * @param flush - If true, the value will be set in the description input as well.
	 */
	set(value: string | undefined, flush?: boolean) {
		if (!this.projectId || !this.branchName) {
			throw new Error('ReactivePRBody not initialized');
		}

		const newValue = value ?? '';

		this._value = newValue;

		if (flush) {
			this._descriptionInput?.setText(newValue);
		}

		// Don't persist the default value
		if (value !== this.getDefaultBody()) {
			setPersistedPRBody(this.projectId, this.branchName, value);
		}
	}

	append(value: string, flush?: boolean) {
		this.set(this._value + value, flush);
	}

	reset() {
		this.set(undefined);
	}

	get descriptionInput() {
		return this._descriptionInput;
	}

	set descriptionInput(value: ReturnType<typeof MessageEditor> | undefined) {
		this._descriptionInput = value;
	}

	get templateBody() {
		return this._templateBody;
	}

	set templateBody(value: string | undefined) {
		const currentBody = this._value;
		const currentDefaultBody = this.getDefaultBody();

		this._templateBody = value;

		// If the current body is either empty or the default body,
		// set the body to the new template body.
		if (
			currentBody === undefined ||
			isEmptyLine(currentBody) ||
			currentBody === currentDefaultBody
		) {
			const defaultBody = this.getDefaultBody();
			this.set(defaultBody, true);
		}
	}
}
