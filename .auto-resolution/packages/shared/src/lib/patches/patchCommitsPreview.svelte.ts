import { inject } from '$lib/context';
import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { mapL } from '$lib/network/loadable';
import { patchEventsSelectors } from '$lib/patchEvents/patchEventsSlice';
import { createPatchEventChannelKey, type LoadablePatchEventChannel } from '$lib/patchEvents/types';
import { PATCH_COMMIT_SERVICE } from '$lib/patches/patchCommitService';
import { patchCommitTable } from '$lib/patches/patchCommitsSlice';
import { getPatchIdable } from '$lib/patches/patchIdablesPreview.svelte';
import { patchSectionsSelectors } from '$lib/patches/patchSectionsSlice';
import { APP_STATE, type AppPatchEventsState } from '$lib/redux/store.svelte';
import type { Loadable } from '$lib/network/types';
import type { PatchEventsService } from '$lib/patchEvents/patchEventsService';
import type { LoadablePatchCommit, Section } from '$lib/patches/types';
import type { Reactive } from '$lib/storeUtils';

export function getPatch(
	branchUuid: string,
	changeId: string,
	inView?: InView
): Reactive<LoadablePatchCommit | undefined> {
	const patchService = inject(PATCH_COMMIT_SERVICE);
	const appState = inject(APP_STATE);
	const patchInterest = patchService.getPatchWithSectionsInterest(branchUuid, changeId);
	registerInterest(patchInterest, inView);

	const patch = $derived(patchCommitTable.selectors.selectById(appState.patches, changeId));

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
	const patchService = inject(PATCH_COMMIT_SERVICE);
	const appState = inject(APP_STATE);
	const patchInterest = patchService.getPatchWithSectionsInterest(branchUuid, changeId);
	registerInterest(patchInterest, inView);

	const patch = $derived(patchCommitTable.selectors.selectById(appState.patches, changeId));
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
): Reactive<Loadable<Section[]>> {
	const appState = inject(APP_STATE);

	const patch = getPatchIdable(branchUuid, changeId, oldVersion, newVersion);
	const sections = $derived(
		mapL(patch.current, (patch) => {
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
