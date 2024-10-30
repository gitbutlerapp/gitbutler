<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { stackingFeature, stackingFeatureMultipleSeries } from '$lib/config/uiFeatureFlags';
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
</script>

<SettingsPage title="Experimental features">
	<p class="text-13 text-body experimental-settings__text">
		This sections contains a list of feature flags for features that are still in development or in
		an experimental stage.
	</p>

	<div class="experimental-settings__toggles">
		<div class="stack-v">
			<SectionCard labelFor="stackingFeature" orientation="row" roundedBottom={false}>
				<svelte:fragment slot="title">Branch stacking UI</svelte:fragment>
				<svelte:fragment slot="caption">
					Enables the new user interface for managing lanes / stacks of branches.
				</svelte:fragment>
				<svelte:fragment slot="actions">
					<Toggle
						id="stackingFeature"
						checked={$stackingFeature}
						onclick={() => ($stackingFeature = !$stackingFeature)}
					/>
				</svelte:fragment>
			</SectionCard>
			<SectionCard
				labelFor="stackingFeatureMultipleSeries"
				orientation="row"
				roundedTop={false}
				disabled={!$stackingFeature}
			>
				<svelte:fragment slot="title">Branch stacking multiple series</svelte:fragment>
				<svelte:fragment slot="caption">
					Experimental support for using the new stacking interface to create multiple branches per
					lane / stack. Not all features are supported yet.
				</svelte:fragment>
				<svelte:fragment slot="actions">
					<Toggle
						id="stackingFeatureMultipleSeries"
						checked={$stackingFeatureMultipleSeries}
						onclick={() => ($stackingFeatureMultipleSeries = !$stackingFeatureMultipleSeries)}
					/>
				</svelte:fragment>
			</SectionCard>
		</div>
	</div>
</SettingsPage>

<style>
	.experimental-settings__text {
		color: var(--clr-text-2);
		margin-bottom: 10px;
	}

	.experimental-settings__toggles {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
</style>
