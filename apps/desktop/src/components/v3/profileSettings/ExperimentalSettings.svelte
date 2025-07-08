<script lang="ts">
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { compactWorkspace, ircEnabled, ircServer } from '$lib/config/uiFeatureFlags';
	import { User } from '$lib/user/user';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';

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
	<SectionCard labelFor="v3Design" orientation="row" roundedBottom={false}>
		{#snippet title()}
			V3 Design
		{/snippet}
		{#snippet caption()}
			<p>Enable the new V3 User Interface.</p>
			<p>
				Share your feedback on <Link href="https://discord.gg/MmFkmaJ42D">Discord</Link>, or create
				a <Link href="https://github.com/gitbutlerapp/gitbutler/issues/new?template=BLANK_ISSUE"
					>GitHub issue</Link
				>.
			</p>

			<p class="clr-text-2">Known issues:</p>
			<ul class="clr-text-2">
				<li>- A restart may be needed for the change to fully take effect</li>
				<li>
					- It is currently not possible to assign uncommitted changes to a lane
					<Link href="https://github.com/gitbutlerapp/gitbutler/issues/8637">GitHub Issue</Link>
				</li>
			</ul>
		{/snippet}

		{#snippet actions()}
			<Toggle
				id="v3Design"
				checked={$settingsStore?.featureFlags.v3}
				onclick={() => settingsService.updateFeatureFlags({ v3: !$settingsStore?.featureFlags.v3 })}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard
		labelFor="compact-workspace"
		roundedTop={false}
		roundedBottom={false}
		orientation="row"
	>
		{#snippet title()}
			Compact workspace
		{/snippet}
		{#snippet caption()}
			Show file preview in the same column as the branch or commit.
		{/snippet}

		{#snippet actions()}
			<Toggle
				id="compact-workspace"
				checked={$compactWorkspace}
				onclick={() => compactWorkspace.set(!$compactWorkspace)}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard labelFor="gitbutler-actions" roundedTop={false} orientation="row">
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
			labelFor="irc"
			roundedTop={!$settingsStore?.featureFlags.actions}
			roundedBottom={!$ircEnabled}
			orientation="row"
		>
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
