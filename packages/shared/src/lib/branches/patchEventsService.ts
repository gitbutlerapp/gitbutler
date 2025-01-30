import { upsertPatchEvent } from '$lib/branches/patchEventsSlice';
import {
	type ApiPatchEvent,
	apiToPatchEvent,
	createPatchEventChannelKey,
	type LoadablePatchEventChannel
} from '$lib/branches/types';
import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { POLLING_GLACIALLY } from '$lib/polling';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

export class PatchEventsService {
	private readonly patchEventsInterests = new InterestStore<{
		projectId: string;
		changeId: string;
	}>(POLLING_GLACIALLY);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getPatchEventsInterest(projectId: string, changeId: string): Interest {
		return this.patchEventsInterests
			.findOrCreateSubscribable({ projectId, changeId }, async () => {
				const patchEventChannelKey = createPatchEventChannelKey(projectId, changeId);
				try {
					const apiPatchEvents = await this.httpClient.get<ApiPatchEvent[]>(
						`patch_events/${projectId}/patch/${changeId}`
					);

					// Return the events in reverse order so that
					// the newest events are at the bottom
					apiPatchEvents.reverse();

					const events = apiPatchEvents.map(apiToPatchEvent);
					const patchEventChannel: LoadablePatchEventChannel = {
						status: 'found',
						id: patchEventChannelKey,
						value: {
							id: patchEventChannelKey,
							projectId,
							changeId,
							events
						}
					};

					this.appDispatch.dispatch(upsertPatchEvent(patchEventChannel));
				} catch (error: unknown) {
					this.appDispatch.dispatch(
						upsertPatchEvent(
							errorToLoadable(error, createPatchEventChannelKey(projectId, changeId))
						)
					);
				}
			})
			.createInterest();
	}

	async refreshPatchEvents(projectId: string, changeId: string): Promise<void> {
		await this.patchEventsInterests.invalidate({ projectId, changeId });
	}
}
