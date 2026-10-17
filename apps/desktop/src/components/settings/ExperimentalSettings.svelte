<script lang="ts">
	import { BACKEND } from "$lib/backend";
	import { fModeEnabled } from "$lib/config/uiFeatureFlags";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { inject } from "@gitbutler/core/context";
	import { Button, CardGroup, Toggle } from "@gitbutler/ui-svelte";

	const backend = inject(BACKEND);
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = settingsService.appSettings;
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

	<CardGroup.Item labelFor="worktree-manipulation">
		{#snippet title()}
			Worktree manipulation
		{/snippet}
		{#snippet caption()}
			Enable experimental support for working with linked git worktrees.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="worktree-manipulation"
				checked={$settingsStore?.featureFlags.worktreeManipulation}
				onclick={() =>
					settingsService.updateFeatureFlags({
						worktreeManipulation: !$settingsStore?.featureFlags.worktreeManipulation,
					})}
			/>
		{/snippet}
	</CardGroup.Item>

	{#if backend.platformName !== "web"}
		<CardGroup.Item labelFor="lite-frontend">
			{#snippet title()}
				Lite interface
			{/snippet}
			{#snippet caption()}
				Use the new Lite interface instead of this one. Takes effect after a restart.
			{/snippet}
			{#snippet actions()}
				<div class="flex gap-8 items-center">
					{#if $settingsStore?.featureFlags.liteFrontend}
						<Button kind="outline" onclick={() => backend.relaunch()}>Restart</Button>
					{/if}
					<Toggle
						id="lite-frontend"
						checked={$settingsStore?.featureFlags.liteFrontend}
						onclick={() =>
							settingsService.updateFeatureFlags({
								liteFrontend: !$settingsStore?.featureFlags.liteFrontend,
							})}
					/>
				</div>
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
