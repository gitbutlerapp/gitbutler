<script lang="ts">
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import {
		ircEnabled,
		ircServer,
		fModeEnabled,
		useNewRebaseEngine
	} from '$lib/config/uiFeatureFlags';
	import { USER } from '$lib/user/user';
	import { inject } from '@gitbutler/core/context';
	import { CardGroup, Textbox, Toggle } from '@gitbutler/ui';

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

	{#if $user?.role === 'admin'}
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
							singleBranch: !$settingsStore?.featureFlags.singleBranch
						})}
				/>
			{/snippet}
		</CardGroup.Item>

		<CardGroup.Item labelFor="irc">
			{#snippet title()}
				IRC
			{/snippet}
			{#snippet caption()}
				Enable experimental in-app chat.
			{/snippet}
			{#snippet actions()}
				<Toggle id="irc" checked={$ircEnabled} onclick={() => ($ircEnabled = !$ircEnabled)} />
			{/snippet}
		</CardGroup.Item>
		{#if $ircEnabled}
			<CardGroup.Item>
				<Textbox
					value={$ircServer}
					size="large"
					label="Server"
					placeholder="wss://irc.gitbutler.com:443"
					onchange={(value) => ($ircServer = value)}
				/>
			</CardGroup.Item>
		{/if}
	{/if}
</CardGroup>

<style>
	.experimental-settings__text {
		margin-bottom: 10px;
		color: var(--clr-text-2);
	}
</style>
