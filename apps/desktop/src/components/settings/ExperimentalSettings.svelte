<script lang="ts">
	import { fModeEnabled, useNewRebaseEngine } from "$lib/config/uiFeatureFlags";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { USER } from "$lib/user/user";
	import { inject } from "@gitbutler/core/context";
	import { CardGroup, Toggle } from "@gitbutler/ui";

	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = settingsService.appSettings;

	const user = inject(USER);
</script>

<p class="text-12 text-body experimental-settings__text">
	Flags for features in development or beta. Features may not work fully.
	<br />
	Use at your own risk.
</p>

<CardGroup>
	<CardGroup.Item labelFor="apply3">
		{#snippet title()}
			New apply to workspace
		{/snippet}
		{#snippet caption()}
			Use the V3 version of apply and unapply operations for workspace changes.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="apply3"
				checked={$settingsStore?.featureFlags.apply3}
				onclick={() =>
					settingsService.updateFeatureFlags({ apply3: !$settingsStore?.featureFlags.apply3 })}
			/>
		{/snippet}
	</CardGroup.Item>
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
	<CardGroup.Item labelFor="new-rebase-engine">
		{#snippet title()}
			New rebase engine
		{/snippet}
		{#snippet caption()}
			Use the new graph-based rebase engine for stack operations.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="new-rebase-engine"
				checked={$useNewRebaseEngine}
				onclick={() => useNewRebaseEngine.set(!$useNewRebaseEngine)}
			/>
		{/snippet}
	</CardGroup.Item>

	{#if $user?.role === "admin"}
		<CardGroup.Item labelFor="single-branch">
			{#snippet title()}
				Single-branch mode
			{/snippet}
			{#snippet caption()}
				Stay in the workspace view when leaving the gitbutler/workspace branch.
			{/snippet}
			{#snippet actions()}
				<Toggle
					id="single-branch"
					checked={$settingsStore?.featureFlags.singleBranch}
					onclick={() =>
						settingsService.updateFeatureFlags({
							singleBranch: !$settingsStore?.featureFlags.singleBranch,
						})}
				/>
			{/snippet}
		</CardGroup.Item>
	{/if}

	<CardGroup.Item labelFor="irc">
		{#snippet title()}
			IRC integration
		{/snippet}
		{#snippet caption()}
			Enable IRC for remote collaboration and automated Claude Code session sharing.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="irc"
				checked={$settingsStore?.featureFlags.irc}
				onclick={() =>
					settingsService.updateFeatureFlags({ irc: !$settingsStore?.featureFlags.irc })}
			/>
		{/snippet}
	</CardGroup.Item>
</CardGroup>

<style>
	.experimental-settings__text {
		margin-bottom: 10px;
		color: var(--text-2);
	}
</style>
