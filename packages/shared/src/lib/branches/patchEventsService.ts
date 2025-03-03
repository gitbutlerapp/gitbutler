import { patchEventsSelectors, upsertPatchEvent } from '$lib/branches/patchEventsSlice';
import {
	type ApiPatchEvent,
	apiToPatchEvent,
	createPatchEventChannelKey,
	isApiPatchEvent,
	type LoadablePatchEventChannel,
	type PatchEvent
} from '$lib/branches/types';
import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable, isFound } from '$lib/network/loadable';
import { playSound } from '$lib/sounds';
import { asyncToSyncSignals, writableDerived } from '$lib/storeUtils';
import { createConsumer } from '@rails/actioncable';
import { type Readable } from 'svelte/store';
import type { PatchService } from '$lib/branches/patchService';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch, AppPatchEventsState } from '$lib/redux/store.svelte';

function getActionCableEndpoint(token: string | undefined, baseUrl: string): string {
	const domain = baseUrl.replace('http', 'ws');
	const url = new URL('cable', domain);

	const urlSearchParams = new URLSearchParams();
	if (token) {
		urlSearchParams.append('token', token);
	}
	url.search = urlSearchParams.toString();

	return url.toString();
}

export class PatchEventsService {
	private userId: number | undefined;
	private chatSoundUrl: string | undefined;
	private readonly patchEventsInterestStore = new InterestStore<{
		changeId: string;
		projectId: string;
	}>();

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appState: AppPatchEventsState,
		private readonly appDispatch: AppDispatch,
		private readonly token: Readable<string | undefined>,
		private readonly patchService: PatchService,
		private readonly websocketBase: string
	) {}

	setUserId(userId: number) {
		this.userId = userId;
	}

	setChatSoundUrl(chatSoundUrl: string) {
		this.chatSoundUrl = chatSoundUrl;
	}

	patchEventsInterest(projectId: string, changeId: string): Interest {
		// Using writableDerived over derived because derived's start stop
		// notifier doesn't behave as expected, and doesn't runs top even
		// if all of the dependencies have unsubscribed.
		const subscription = writableDerived(
			this.token,
			undefined,
			asyncToSyncSignals<[string | undefined]>(async (token: string | undefined) => {
				// Before we subscribe to chat, fetch the initial set of patch data.
				await this.fetchInitialPatchEvents(projectId, changeId);

				const actionCableEndpoint = getActionCableEndpoint(token, this.websocketBase);
				const consumer = createConsumer(actionCableEndpoint);
				consumer.subscriptions.create(
					{ channel: 'ChatChannel', change_id: changeId, project_id: projectId },
					{
						received: (data: unknown) => {
							if (!isApiPatchEvent(data)) return;
							this.handlePatchEventData(projectId, changeId, data);
						}
					}
				);

				return async () => {
					consumer.disconnect();
				};
			})
		);

		return this.patchEventsInterestStore
			.findOrCreateSubscribable({ projectId, changeId }, () => {
				const unsubscribe = subscription.subscribe(() => {});

				return () => {
					unsubscribe();
				};
			})
			.createInterest();
	}

	private shouldPlayChatSound(patchEvent: PatchEvent): boolean {
		if (patchEvent.user?.id === undefined || this.userId === undefined) {
			return false;
		}

		return patchEvent.user.id !== this.userId && patchEvent.eventType === 'chat';
	}

	private handlePatchEventData(projectId: string, changeId: string, data: ApiPatchEvent) {
		const key = createPatchEventChannelKey(projectId, changeId);
		const eventChannel = patchEventsSelectors.selectById(this.appState.patchEvents, key);
		// If the event is not found then it's either not found, loading, or
		// unrequired by the frontend.
		if (!isFound(eventChannel)) return;
		const mutableEventChannel = structuredClone(eventChannel);
		const patchEvent = apiToPatchEvent(data);
		// Failed to parse the patch event.
		if (!patchEvent) return;
		if (!mutableEventChannel.value.events.some((event) => event.uuid === patchEvent.uuid)) {
			mutableEventChannel.value.events.unshift(patchEvent);
			this.appDispatch.dispatch(upsertPatchEvent(mutableEventChannel));
		}

		// If a chat event has appeared, then we want to make sure that the
		// change is propogated elsewhere.
		if (data.event_type === 'patch_version' || data.event_type === 'issue_status') {
			this.patchService.refreshPatchWithSections(changeId);
		}

		if (this.shouldPlayChatSound(patchEvent) && this.chatSoundUrl) {
			playSound(this.chatSoundUrl);
		}
	}

	private async fetchInitialPatchEvents(projectId: string, changeId: string) {
		try {
			const apiPatchEvents = await this.httpClient.get<ApiPatchEvent[]>(
				`patch_events/${projectId}/patch/${changeId}`
			);

			// Return the events in reverse order so that
			// the newest events are at the bottom
			apiPatchEvents.reverse();

			const events = apiPatchEvents.map(apiToPatchEvent).filter((e): e is PatchEvent => !!e);
			console.log(events);
			const patchEventChannel: LoadablePatchEventChannel = createPatchEventChannel(
				projectId,
				changeId,
				events
			);

			this.appDispatch.dispatch(upsertPatchEvent(patchEventChannel));
		} catch (error: unknown) {
			this.appDispatch.dispatch(
				upsertPatchEvent(errorToLoadable(error, createPatchEventChannelKey(projectId, changeId)))
			);
		}
	}
}

function createPatchEventChannel(
	projectId: string,
	changeId: string,
	events: PatchEvent[]
): LoadablePatchEventChannel {
	const patchEventChannelKey = createPatchEventChannelKey(projectId, changeId);
	return {
		status: 'found',
		id: patchEventChannelKey,
		value: {
			id: patchEventChannelKey,
			projectId,
			changeId,
			events
		}
	};
}
