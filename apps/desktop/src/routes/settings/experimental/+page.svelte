<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import {
		featureBaseBranchSwitching,
		featureInlineUnifiedDiffs,
		stackingFeature,
		featureTopics
	} from '$lib/config/uiFeatureFlags';
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';

	const baseBranchSwitching = featureBaseBranchSwitching();
	const inlineUnifiedDiffs = featureInlineUnifiedDiffs();
	const topicsEnabled = featureTopics();
</script>

<SettingsPage title="Experimental features">
	<p class="text-13 text-body experimental-settings__text">
		This sections contains a list of feature flags for features that are still in development or in
		an experimental stage.
	</p>
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
	<SectionCard labelFor="inlineUnifiedDiffs" orientation="row">
		<svelte:fragment slot="title">Display word diffs inline</svelte:fragment>
		<svelte:fragment slot="caption">
			Rather than showing one line which is the all the removals and another line which is all the
			additions, with the specific words emboldened. With the feature flag enabled, only one line
			with both the words added and removed is displayed.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="inlineUnifiedDiffs"
				checked={$inlineUnifiedDiffs}
				on:click={() => ($inlineUnifiedDiffs = !$inlineUnifiedDiffs)}
			/>
		</svelte:fragment>
	</SectionCard>
	<SectionCard labelFor="stackingFeature" orientation="row">
		<svelte:fragment slot="title">Branch stacking</svelte:fragment>
		<svelte:fragment slot="caption">
			Allows for branch / pull request stacking. The user interface for this is still very crude.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="stackingFeature"
				checked={$stackingFeature}
				on:click={() => ($stackingFeature = !$stackingFeature)}
			/>
		</svelte:fragment>
	</SectionCard>
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
</SettingsPage>

<style>
	.experimental-settings__text {
		color: var(--clr-text-2);
		margin-bottom: 12px;
	}
</style>
