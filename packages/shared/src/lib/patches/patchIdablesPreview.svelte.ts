import { inject } from '$lib/context';
import { registerInterest } from '$lib/interest/registerInterestFunction.svelte';
import { PATCH_IDABLE_SERVICE } from '$lib/patches/patchIdableService';
import { patchIdableTable } from '$lib/patches/patchIdablesSlice';
import { patchIdableId, type LoadablePatchIdable } from '$lib/patches/types';
import { reactive } from '$lib/reactiveUtils.svelte';
import { APP_STATE } from '$lib/redux/store.svelte';
import { type Reactive } from '$lib/storeUtils';

export function getPatchIdable(
	branchUuid: string,
	changeId: string,
	oldVersion: number | undefined,
	newVersion: number
): Reactive<LoadablePatchIdable | undefined> {
	const appState = inject(APP_STATE);
	const patchIdableService = inject(PATCH_IDABLE_SERVICE);

	const interest = patchIdableService.getPatchIdableInterest({
		branchUuid,
		changeId,
		oldVersion,
		newVersion
	});
	registerInterest(interest);

	const key = patchIdableId({ branchUuid, changeId, oldVersion, newVersion });
	const current = $derived(patchIdableTable.selectors.selectById(appState.patchIdables, key));
	return reactive(() => current);
}
