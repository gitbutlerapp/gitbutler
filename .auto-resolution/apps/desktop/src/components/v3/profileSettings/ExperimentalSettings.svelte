<script lang="ts">
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { User } from '$lib/user/user';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
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
		<SectionCard orientation="row">
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
		gap: 16px;
	}
</style>
