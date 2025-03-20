import { reactive, type Reactive } from '@gitbutler/shared/storeUtils';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { goto } from '$app/navigation';
import { page } from '$app/state';

/**
 * Sets the interdiff before version.
 *
 * If you are setting both the before and after versions, make sure to await
 * these calls as otherwise one will overwrite the other.
 */
export async function setBeforeVersion(before: number) {
	const params = new URLSearchParams(location.search);
	if (before === -1) {
		params.delete('beforeVersion');
	} else {
		params.set('beforeVersion', before.toString());
	}
	await goto(`?${params.toString()}`, { replaceState: true, noScroll: true, keepFocus: true });
}

/**
 * Sets the interdiff after version.
 *
 * If you are setting both the before and after versions, make sure to await
 * these calls as otherwise one will overwrite the other.
 */
export async function setAfterVersion(latestVersion: number, after: number) {
	const params = new URLSearchParams(location.search);
	if (after === latestVersion) {
		params.delete('afterVersion');
	} else {
		params.set('afterVersion', after.toString());
	}
	await goto(`?${params.toString()}`, { replaceState: true, noScroll: true, keepFocus: true });
}

export function getBeforeVersion(): Reactive<number> {
	const current = $derived.by(() => {
		const param = page.url.searchParams.get('beforeVersion');
		console.log(param);
		if (isDefined(param)) {
			return parseInt(param);
		} else {
			return -1;
		}
	});
	return reactive(() => current);
}

export function getAfterVersion(latestVersion: number): Reactive<number> {
	const current = $derived.by(() => {
		const param = page.url.searchParams.get('afterVersion');
		if (isDefined(param)) {
			return parseInt(param);
		} else {
			return latestVersion;
		}
	});
	return reactive(() => current);
}
