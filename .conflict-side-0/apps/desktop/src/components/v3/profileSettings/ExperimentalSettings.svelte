<script lang="ts">
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { ircEnabled, ircServer } from '$lib/config/uiFeatureFlags';
	import { User } from '$lib/user/user';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';

	const settingsService = getContext(SettingsService);
	const settingsStore = settingsService.appSettings;

	const user = getContextStore(User);
</script>

<p class="text-12 text-body experimental-settings__text">
	This section contains a list of feature flags for features that are still in development or in
	beta.
	<br />
	Some of these features may not be fully functional or may have bugs. Use them at your own risk.
</p>

<div class="experimental-settings__toggles">
	{#if $user?.role === 'admin'}
		<SectionCard roundedBottom={false} orientation="row">
			{#snippet title()}
				v3 Design
			{/snippet}
			{#snippet caption()}
				Enable the new v3 User Interface.
			{/snippet}

			{#snippet actions()}
				<Toggle
					id="v3Design"
					checked={$settingsStore?.featureFlags.v3}
					onclick={() =>
						settingsService.updateFeatureFlags({ v3: !$settingsStore?.featureFlags.v3 })}
				/>
			{/snippet}
		</SectionCard>
		<SectionCard roundedTop={false} roundedBottom={!$ircEnabled} orientation="row">
			{#snippet title()}
				IRC
			{/snippet}
			{#snippet caption()}
				Enable experimental in-app chat.
			{/snippet}
			{#snippet actions()}
				<Toggle id="irc" checked={$ircEnabled} onclick={() => ($ircEnabled = !$ircEnabled)} />
				{#if $ircEnabled}{/if}
			{/snippet}
		</SectionCard>
		{#if $ircEnabled}
			<SectionCard roundedTop={false} topDivider orientation="column">
				{#snippet actions()}
					<Textbox
						value={$ircServer}
						size="large"
						label="Server"
						placeholder="wss://irc.gitbutler.com:443"
						onchange={(value) => ($ircServer = value)}
					/>
				{/snippet}
			</SectionCard>
		{/if}
	{/if}
</div>

<style>
	.experimental-settings__text {
		color: var(--clr-text-2);
		margin-bottom: 10px;
	}

	.experimental-settings__toggles {
		display: flex;
		flex-direction: column;
	}
</style>
