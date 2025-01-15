import { patchSectionsSelectors } from '$lib/branches/patchSectionsSlice';
import { patchesSelectors } from '$lib/branches/patchesSlice';
import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import type { PatchService } from '$lib/branches/patchService';
import type { LoadablePatch, Section } from '$lib/branches/types';
import type { AppPatchesState, AppPatchSectionsState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';

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

	const patch = $derived(patchesSelectors.selectById(appState.patches, branchUuid));
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
