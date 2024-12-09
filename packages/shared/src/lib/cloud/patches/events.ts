import { CloudPatch, type ApiPatch } from '$lib/cloud/types';
import { derived, type Readable } from 'svelte/store';
import type { HttpClient } from '$lib/httpClient';

interface ApiChatUser {
	id: number;
	login: string;
	name: string;
	email: string;
	avatar_url?: string;
}

class CloudChatUser {
	id: number;
	login: string;
	name: string;
	email: string;
	avatarUrl?: string;

	constructor(apiChatUser: ApiChatUser) {
		this.id = apiChatUser.id;
		this.login = apiChatUser.login;
		this.name = apiChatUser.name;
		this.email = apiChatUser.email;
		this.avatarUrl = apiChatUser.avatar_url;
	}
}

interface DiffPatchEntry {
	type: 'context' | 'added' | 'removed';
	left?: number;
	right?: number;
	line: string;
}

interface ApiChatMessage {
	uuid: string;
	user: ApiChatUser;
	outdated: boolean;
	issue: boolean;
	resolved: boolean;
	thread_id?: string;
	comment: unknown;

	// data needed if commenting on part of a patch
	diff_sha?: string;
	range?: string;
	diff_patch_array?: DiffPatchEntry[];

	created_at: string;
	updated_at: string;
}

class CloudChatMessage {
	uuid: string;
	user: CloudChatUser;
	outdated: boolean;
	issue: boolean;
	resolved: boolean;
	threadId?: string;
	comment: unknown;

	// data needed if commenting on part of a patch
	diffSha?: string;
	range?: string;
	diffPatchArray?: DiffPatchEntry[];

	createdAt: Date;
	updatedAt: Date;

	constructor(apiChatMessage: ApiChatMessage) {
		this.uuid = apiChatMessage.uuid;
		this.user = new CloudChatUser(apiChatMessage.user);
		this.outdated = apiChatMessage.outdated;
		this.issue = apiChatMessage.issue;
		this.resolved = apiChatMessage.resolved;
		this.threadId = apiChatMessage.thread_id;
		this.comment = apiChatMessage.comment;
		this.diffSha = apiChatMessage.diff_sha;
		this.range = apiChatMessage.range;
		this.diffPatchArray = apiChatMessage.diff_patch_array;
		this.createdAt = new Date(apiChatMessage.created_at);
		this.updatedAt = new Date(apiChatMessage.updated_at);
	}
}

type EventableTypes = 'chat' | 'issue_status' | 'patch_version' | 'patch_status';

interface ApiPatchEvent<T extends EventableTypes = EventableTypes> {
	uuid: string;
	user: ApiChatUser;
	event_type: T;
	eventable: T extends 'chat' | 'issue_status' ? ApiChatMessage : ApiPatch;
}

class _CloudPatchEvent<T extends EventableTypes> {
	uuid: string;
	user: CloudChatUser;
	eventType: T;
	eventable: T extends 'chat' | 'issue_status' ? CloudChatMessage : CloudPatch;

	constructor(apiPatchEvent: ApiPatchEvent<T>) {
		this.uuid = apiPatchEvent.uuid;
		this.user = new CloudChatUser(apiPatchEvent.user);
		this.eventType = apiPatchEvent.event_type;
		if (this.eventType === 'chat' || this.eventType === 'issue_status') {
			// @ts-expect-error For some reason the conditional on T does not infer correctly
			this.eventable = new CloudChatMessage(apiPatchEvent.eventable as ApiChatMessage);
		} else {
			// @ts-expect-error For some reason the conditional on T does not infer correctly
			this.eventable = new CloudPatch(apiPatchEvent.eventable as ApiPatch);
		}
	}
}

export class ApiPatchEventsService {
	readonly canGetEvents: Readable<boolean>;
	readonly canSubscribeToEvents: Readable<boolean>;
	readonly canCreateEvents: Readable<boolean>;

	constructor(
		private readonly httpClient: HttpClient,
		readonly token: Readable<string | undefined>
	) {
		this.canGetEvents = httpClient.authenticationAvailable;
		this.canSubscribeToEvents = derived(token, (token) => !!token);
		this.canCreateEvents = httpClient.authenticationAvailable;
	}

	async getEvents(repositoryId: string, changeId: string): Promise<ApiPatchEvent[] | undefined> {
		try {
			return await this.httpClient.get<ApiPatchEvent[]>(`${repositoryId}/patch/${changeId}`);
		} catch (e: unknown) {
			// If the internet is down, silently fail
			if (e instanceof TypeError) {
				return undefined;
			} else {
				throw e;
			}
		}
	}
}

export class CloudPatchEventsService {
	// private readonly apiPatchEvents: Writable<Set<ApiPatchEvent>>;
	// readonly patchEvents: Readable<CloudPatchEvent[]>;
}
