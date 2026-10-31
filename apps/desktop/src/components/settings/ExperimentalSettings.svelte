<script lang="ts">
	import { BACKEND } from "$lib/backend";
	import { fModeEnabled } from "$lib/config/uiFeatureFlags";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { inject } from "@gitbutler/core/context";
	import { CardGroup, Toggle } from "@gitbutler/ui-svelte";

	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = settingsService.appSettings;
	const backend = inject(BACKEND);

	// Only the desktop app's nightly and release builds include the native app.
	let hasNativeApp = $state(false);
	if (backend.platformName !== "web") {
		backend
			.getAppInfo()
			.then((info) => (hasNativeApp = ["GitButler", "GitButler Nightly"].includes(info.name)));
	}
</script>

<p class="text-12 text-body experimental-settings__text">
	Flags for features in development or beta. Features may not work fully.
	<br />
	Use at your own risk.
</p>

<CardGroup>
	<CardGroup.Item labelFor="f-mode">
		{#snippet title()}
			F Mode Navigation
		{/snippet}
		{#snippet caption()}
			Enable F mode for quick keyboard navigation to buttons using two-letter shortcuts.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="f-mode"
				checked={$fModeEnabled}
				onclick={() => fModeEnabled.set(!$fModeEnabled)}
			/>
		{/snippet}
	</CardGroup.Item>

	{#if hasNativeApp}
		<CardGroup.Item labelFor="native-app">
			{#snippet title()}
				Native app
			{/snippet}
			{#snippet caption()}
				Open GitButler's experimental native app instead of this one. Takes effect after you restart
				GitButler. To come back, turn off "Open this app at start" in the native app's settings.
			{/snippet}
			{#snippet actions()}
				<Toggle
					id="native-app"
					checked={$settingsStore?.featureFlags.gpui}
					onclick={() =>
						settingsService.updateFeatureFlags({
							gpui: !$settingsStore?.featureFlags.gpui,
						})}
				/>
			{/snippet}
		</CardGroup.Item>
	{/if}
</CardGroup>

<style>
	.experimental-settings__text {
		margin-bottom: 10px;
		color: var(--text-2);
	}
</style>
