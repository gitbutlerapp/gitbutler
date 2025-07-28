<script lang="ts">
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { ircEnabled, ircServer, workspaceRulesEnabled } from '$lib/config/uiFeatureFlags';
	import { USER } from '$lib/user/user';
	import { inject } from '@gitbutler/shared/context';
	import { SectionCard, Spacer, Textbox, Toggle } from '@gitbutler/ui';

	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = settingsService.appSettings;

	const user = inject(USER);
</script>

<p class="text-12 text-body experimental-settings__text">
	This section contains a list of feature flags for features that are still in development or in
	beta.
	<br />
	Some of these features may not be fully functional or may have bugs. Use them at your own risk.
</p>

<div class="experimental-settings__toggles">
	<SectionCard labelFor="gitbutler-actions" roundedTop roundedBottom={false} orientation="row">
		{#snippet title()}
			GitButler Actions
		{/snippet}
		{#snippet caption()}
			Enable the GitButler Actions log
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="gitbutler-actions"
				checked={$settingsStore?.featureFlags.actions}
				onclick={() =>
					settingsService.updateFeatureFlags({ actions: !$settingsStore?.featureFlags.actions })}
			/>
		{/snippet}
	</SectionCard>
	<SectionCard labelFor="ws3" roundedTop={false} orientation="row">
		{#snippet title()}
			New workspace backend
		{/snippet}
		{#snippet caption()}
			Enable this to use the new API for rendering the workspace state. Enabling this should be
			functionally identical to the old API but it resolves a class of bugs that were present in the
			old implementation.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="ws3"
				checked={$settingsStore?.featureFlags.ws3}
				onclick={() =>
					settingsService.updateFeatureFlags({ ws3: !$settingsStore?.featureFlags.ws3 })}
			/>
		{/snippet}
	</SectionCard>

	{#if $user?.role === 'admin'}
		<Spacer margin={20} />
		{#if $settingsStore?.featureFlags.actions}
			<SectionCard labelFor="butbot" roundedTop roundedBottom={false} orientation="row">
				{#snippet title()}
					butbot
				{/snippet}
				{#snippet caption()}
					Enable the butbot chat.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="butbot"
						checked={$settingsStore?.featureFlags.butbot}
						onclick={() =>
							settingsService.updateFeatureFlags({ butbot: !$settingsStore?.featureFlags.butbot })}
					/>
				{/snippet}
			</SectionCard>
		{/if}

		<SectionCard
			labelFor="rules"
			roundedTop={!$settingsStore?.featureFlags.actions}
			roundedBottom={false}
			orientation="row"
		>
			{#snippet title()}
				Workspace Rules
			{/snippet}
			{#snippet caption()}
				Go full dominatrix on your workspace and add a bunch rules that can automatically trigger
				actions.
			{/snippet}
			{#snippet actions()}
				<Toggle
					id="rules"
					checked={$workspaceRulesEnabled}
					onclick={() => workspaceRulesEnabled.set(!$workspaceRulesEnabled)}
				/>
			{/snippet}
		</SectionCard>

		<SectionCard labelFor="irc" roundedTop={false} roundedBottom={!$ircEnabled} orientation="row">
			{#snippet title()}
				IRC
			{/snippet}
			{#snippet caption()}
				Enable experimental in-app chat.
			{/snippet}
			{#snippet actions()}
				<Toggle id="irc" checked={$ircEnabled} onclick={() => ($ircEnabled = !$ircEnabled)} />
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
		margin-bottom: 10px;
		color: var(--clr-text-2);
	}

	.experimental-settings__toggles {
		display: flex;
		flex-direction: column;
	}
</style>
