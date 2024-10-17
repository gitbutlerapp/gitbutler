<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import {
		featureBaseBranchSwitching,
		stackingFeature,
		stackingFeatureMultipleSeries,
		featureTopics
	} from '$lib/config/uiFeatureFlags';
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';

	const baseBranchSwitching = featureBaseBranchSwitching();
	const topicsEnabled = featureTopics();
</script>

<SettingsPage title="Experimental features">
	<p class="text-13 text-body experimental-settings__text">
		This sections contains a list of feature flags for features that are still in development or in
		an experimental stage.
	</p>

	<div class="experimental-settings__toggles">
		<SectionCard labelFor="baseBranchSwitching" orientation="row">
			<svelte:fragment slot="title">Switching the target branch</svelte:fragment>
			<svelte:fragment slot="caption">
				This allows changing of the target branch after the initial project setup from within the
				project settings. Not fully tested yet, use with caution.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="baseBranchSwitching"
					checked={$baseBranchSwitching}
					on:click={() => ($baseBranchSwitching = !$baseBranchSwitching)}
				/>
			</svelte:fragment>
		</SectionCard>

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
						on:click={() => ($stackingFeature = !$stackingFeature)}
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
						on:click={() => ($stackingFeatureMultipleSeries = !$stackingFeatureMultipleSeries)}
					/>
				</svelte:fragment>
			</SectionCard>
		</div>
		<SectionCard labelFor="topics" orientation="row">
			<svelte:fragment slot="title">Topics</svelte:fragment>
			<svelte:fragment slot="caption">
				A highly experimental form of note taking / conversation. The form & function may change
				drastically, and may result in lost notes.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="topics"
					checked={$topicsEnabled}
					on:click={() => ($topicsEnabled = !$topicsEnabled)}
				/>
			</svelte:fragment>
		</SectionCard>
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
