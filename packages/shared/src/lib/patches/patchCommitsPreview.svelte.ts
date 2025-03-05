import { patchEventsSelectors } from '../patchEvents/patchEventsSlice';
import { getContext } from '$lib/context';
import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { map } from '$lib/network/loadable';
import { createPatchEventChannelKey, type LoadablePatchEventChannel } from '$lib/patchEvents/types';
import { PatchCommitService } from '$lib/patches/patchCommitService';
import { patchCommitsSelector } from '$lib/patches/patchCommitsSlice';
import { getPatchIdable } from '$lib/patches/patchIdablesPreview.svelte';
import { patchSectionsSelectors } from '$lib/patches/patchSectionsSlice';
import { AppState, type AppPatchesState, type AppPatchEventsState } from '$lib/redux/store.svelte';
import type { LoadablePatchCommit, Section } from '$lib/patches/types';
import type { Reactive } from '$lib/storeUtils';
import type { PatchEventsService } from '../patchEvents/patchEventsService';

export function getPatch(
	appState: AppPatchesState,
	patchService: PatchCommitService,
	branchUuid: string,
	changeId: string,
	inView?: InView
): Reactive<LoadablePatchCommit | undefined> {
	const patchInterest = patchService.getPatchWithSectionsInterest(branchUuid, changeId);
	registerInterest(patchInterest, inView);

	const patch = $derived(patchCommitsSelector.selectById(appState.patches, changeId));

	return {
		get current() {
			return patch;
		}
	};
}

export function getPatchSections(
	branchUuid: string,
	changeId: string,
	inView?: InView
): Reactive<Section[] | undefined> {
	const patchService = getContext(PatchCommitService);
	const appState = getContext(AppState);
	const patchInterest = patchService.getPatchWithSectionsInterest(branchUuid, changeId);
	registerInterest(patchInterest, inView);

	const patch = $derived(patchCommitsSelector.selectById(appState.patches, changeId));
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

export function getPatchIdableSections(
	branchUuid: string,
	changeId: string,
	oldVersion: number | undefined,
	newVersion: number
): Reactive<Section[] | undefined> {
	const appState = getContext(AppState);

	const patch = getPatchIdable(branchUuid, changeId, oldVersion, newVersion);
	const sections = $derived(
		map(patch.current, (patch) => {
			return (patch.sectionIds || [])
				.map((id) => patchSectionsSelectors.selectById(appState.patchSections, id))
				.filter((a) => a) as Section[];
		})
	);

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
