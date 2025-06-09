<script lang="ts">
	import Section from '$components/Section.svelte';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import {
		assignmentEnabled,
		confettiEnabled,
		workspaceSwapPanels,
		ircEnabled,
		ircServer
	} from '$lib/config/uiFeatureFlags';
	import { User } from '$lib/user/user';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import RadioButton from '@gitbutler/ui/RadioButton.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import { onMount } from 'svelte';

	const settingsService = getContext(SettingsService);
	const settingsStore = settingsService.appSettings;

	const user = getContextStore(User);

	let panelsForm = $state<HTMLFormElement>();

	// Get the value from the store to match the type
	let selectedType: 'dont-swap-panels' | 'swap-middle-to-right' | 'swap-middle-to-left' =
		$workspaceSwapPanels;

	onMount(() => {
		if (panelsForm) {
			panelsForm.panelsSwapMode.value = selectedType;
		}
	});

	function onPanelsFormChange(form: HTMLFormElement) {
		const formData = new FormData(form);
		selectedType = formData.get('panelsSwapMode') as
			| 'dont-swap-panels'
			| 'swap-middle-to-right'
			| 'swap-middle-to-left';
		workspaceSwapPanels.set(selectedType);
		if (panelsForm) {
			panelsForm.panelsSwapMode.value = selectedType;
		}
	}
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

	<SectionCard labelFor="assignments" roundedTop={false} roundedBottom={false} orientation="row">
		{#snippet title()}
			Assign uncommitted changes
		{/snippet}
		{#snippet caption()}
			When enabled you can assign uncommitted changes to branches (stacks).
		{/snippet}

		{#snippet actions()}
			<Toggle
				id="assignments"
				checked={$assignmentEnabled}
				onclick={() => assignmentEnabled.set(!$assignmentEnabled)}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard labelFor="confetti" roundedTop={false} roundedBottom={false} orientation="row">
		{#snippet title()}
			Confetti
		{/snippet}
		{#snippet caption()}
			Mom's spaghetti, who want's some confetti? 🎉
		{/snippet}

		{#snippet actions()}
			<Toggle
				id="confetti"
				checked={$confettiEnabled}
				onclick={() => confettiEnabled.set(!$confettiEnabled)}
			/>
		{/snippet}
	</SectionCard>

	<Spacer margin={20} />

	<Section>
		{#snippet title()}
			Swap workspace panels
		{/snippet}
		{#snippet description()}
			Allows you to swap the left and right panels in the workspace.
		{/snippet}

		<form
			bind:this={panelsForm}
			class="workspace=panels-form"
			onchange={(e) => onPanelsFormChange(e.currentTarget as HTMLFormElement)}
		>
			<SectionCard
				labelFor="dont-swap-panels"
				roundedTop={true}
				roundedBottom={false}
				orientation="row"
			>
				{#snippet title()}
					Don't swap panels
				{/snippet}

				{#snippet actions()}
					<RadioButton name="panelsSwapMode" id="dont-swap-panels" value="dont-swap-panels" />
				{/snippet}
			</SectionCard>
			<SectionCard
				labelFor="swap-middle-to-right"
				roundedTop={false}
				roundedBottom={$user?.role !== 'admin'}
				orientation="row"
			>
				{#snippet title()}
					Middle to right
				{/snippet}

				{#snippet actions()}
					<RadioButton
						name="panelsSwapMode"
						id="swap-middle-to-right"
						value="swap-middle-to-right"
					/>
				{/snippet}
			</SectionCard>
			<SectionCard
				labelFor="swap-middle-to-left"
				roundedTop={false}
				roundedBottom={true}
				orientation="row"
			>
				{#snippet title()}
					Middle to left
				{/snippet}

				{#snippet actions()}
					<RadioButton name="panelsSwapMode" id="swap-middle-to-left" value="swap-middle-to-left" />
				{/snippet}
			</SectionCard>
		</form>
	</Section>

	{#if $user?.role === 'admin'}
		<Spacer margin={20} />

		<SectionCard labelFor="gitbutler-actions" roundedBottom={false} orientation="row">
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
