import { getContext } from '$lib/context';
import { registerInterest } from '$lib/interest/registerInterestFunction.svelte';
import { PatchIdableService } from '$lib/patches/patchIdableService';
import { patchIdablesSelector } from '$lib/patches/patchIdablesSlice';
import { patchIdableId, type LoadablePatchIdable } from '$lib/patches/types';
import { reactive, type Reactive } from '$lib/storeUtils';
import { AppState } from '$lib/redux/store.svelte';

export function getPatchIdable(
	branchUuid: string,
	changeId: string,
	oldVersion: number | undefined,
	newVersion: number
): Reactive<LoadablePatchIdable | undefined> {
	const appState = getContext(AppState);
	const patchIdableService = getContext(PatchIdableService);

	const interest = patchIdableService.getPatchIdableInterest({
		branchUuid,
		changeId,
		oldVersion,
		newVersion
	});
	registerInterest(interest);

	const key = patchIdableId({ branchUuid, changeId, oldVersion, newVersion });
	const current = $derived(patchIdablesSelector.selectById(appState.patchIdables, key));
	return reactive(() => current);
}
