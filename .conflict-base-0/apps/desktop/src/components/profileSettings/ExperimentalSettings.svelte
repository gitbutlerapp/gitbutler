<script lang="ts">
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { ircEnabled, ircServer, codegenEnabled } from '$lib/config/uiFeatureFlags';
	import { USER } from '$lib/user/user';
	import { inject } from '@gitbutler/core/context';
	import { SectionCard, Spacer, Textbox, Toggle } from '@gitbutler/ui';

	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = settingsService.appSettings;

	const user = inject(USER);
</script>

<p class="text-12 text-body experimental-settings__text">
	Flags for features in development or beta. Features may not work fully.
	<br />
	Use at your own risk.
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
	<SectionCard labelFor="ws3" roundedTop={false} roundedBottom={false} orientation="row">
		{#snippet title()}
			New workspace backend
		{/snippet}
		{#snippet caption()}
			Enable this to use the new API for rendering the workspace state. This should correctly detect
			squash-merged PRs as integrated when updating the workspace.
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
	<SectionCard labelFor="rules" roundedTop={false} roundedBottom={false} orientation="row">
		{#snippet title()}
			Workspace Rules
		{/snippet}
		{#snippet caption()}
			Allows you to create rules for assigning new changes to a specific branch based on a
			condition. Still under development - please let us know what you think!
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="rules"
				checked={$settingsStore?.featureFlags.rules}
				onclick={() =>
					settingsService.updateFeatureFlags({ rules: !$settingsStore?.featureFlags.rules })}
			/>
		{/snippet}
	</SectionCard>
	<SectionCard labelFor="codegen" roundedTop={false} orientation="row">
		{#snippet title()}
			Codegen (Claude Code)
		{/snippet}
		{#snippet caption()}
			Enable AI-powered code generation and editing with Claude.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="codegen"
				checked={$codegenEnabled}
				onclick={() => ($codegenEnabled = !$codegenEnabled)}
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
			labelFor="single-branch"
			roundedTop={false}
			roundedBottom={false}
			orientation="row"
		>
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
