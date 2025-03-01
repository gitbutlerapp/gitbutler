import { patchEventsSelectors } from './patchEventsSlice';
import { patchSectionsSelectors } from '$lib/branches/patchSectionsSlice';
import { patchesSelectors } from '$lib/branches/patchesSlice';
import {
	createPatchEventChannelKey,
	type LoadablePatch,
	type LoadablePatchEventChannel,
	type Section
} from '$lib/branches/types';
import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import type { PatchService } from '$lib/branches/patchService';
import type {
	AppPatchesState,
	AppPatchEventsState,
	AppPatchSectionsState
} from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';
import type { PatchEventsService } from './patchEventsService';

export function getPatch(
	appState: AppPatchesState,
	patchService: PatchService,
	branchUuid: string,
	changeId: string,
	inView?: InView
): Reactive<LoadablePatch | undefined> {
	const patchInterest = patchService.getPatchWithSectionsInterest(branchUuid, changeId);
	registerInterest(patchInterest, inView);

	const patch = $derived(patchesSelectors.selectById(appState.patches, changeId));

	return {
		get current() {
			return patch;
		}
	};
}

export function getPatchSections(
	appState: AppPatchesState & AppPatchSectionsState,
	patchService: PatchService,
	branchUuid: string,
	changeId: string,
	inView?: InView
): Reactive<Section[] | undefined> {
	const patchInterest = patchService.getPatchWithSectionsInterest(branchUuid, changeId);
	registerInterest(patchInterest, inView);

	const patch = $derived(patchesSelectors.selectById(appState.patches, changeId));
	const sections = $derived.by(() => {
		if (patch?.status !== 'found') return;

		return (patch.value.sectionIds || [])
			.map((id) => patchSectionsSelectors.selectById(appState.patchSections, id))
			.filter((a) => a) as Section[];
	});

	return {
		get current() {
			return sections;
		}
	};
}

export function getPatchEvents(
	appState: AppPatchEventsState,
	patchEventsService: PatchEventsService,
	projectId: string,
	changeId: string,
	inView?: InView
): Reactive<LoadablePatchEventChannel | undefined> {
	const patchEventsInterest = patchEventsService.patchEventsInterest(projectId, changeId);
	registerInterest(patchEventsInterest, inView);

	const patchEventChannelKey = createPatchEventChannelKey(projectId, changeId);
	const patchEvents = $derived(
		patchEventsSelectors.selectById(appState.patchEvents, patchEventChannelKey)
	);

	return {
		get current() {
			return patchEvents;
		}
	};
}
