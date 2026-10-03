import { locale, setLocale, type AppLocale } from "$lib/i18n";
import type { UiState } from "$lib/state/uiState.svelte";

/**
 * Wires the persisted uiState locale into the i18n module. The persisted value
 * wins once rehydration has run; before that the OS language is used so the
 * first paint already matches the user's system language.
 */
export function initI18n(uiState: UiState) {
	const systemLocale: AppLocale = navigator.languages?.some((tag) =>
		tag.toLowerCase().startsWith("zh"),
	)
		? "zh-CN"
		: "en";
	setLocale(systemLocale);
	// $effect.root is needed because initI18n runs outside a component context.
	$effect.root(() => {
		$effect(() => {
			setLocale(uiState.global.locale.current);
		});
	});
	// changes made via the i18n module propagate back to uiState
	locale.subscribe((value) => {
		if (value !== uiState.global.locale.current) {
			uiState.global.locale.set(value);
		}
	});
}
