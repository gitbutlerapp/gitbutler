<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import {
		featureBaseBranchSwitching,
		featureEditMode,
		featureInlineUnifiedDiffs
	} from '$lib/config/uiFeatureFlags';
	import ContentWrapper from '$lib/settings/ContentWrapper.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';

	const baseBranchSwitching = featureBaseBranchSwitching();
	const inlineUnifiedDiffs = featureInlineUnifiedDiffs();
	const editMode = featureEditMode();
</script>

<ContentWrapper title="Experimental features">
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
	<SectionCard labelFor="editMode" orientation="row">
		<svelte:fragment slot="title">Edit mode</svelte:fragment>
		<svelte:fragment slot="caption">
			Provides an "Edit patch" button on each commit which puts you into edit mode. Edit mode checks
			out a particular commit so you can make changes to a particular commit, and then have the
			child commits automatically rebased on top of the new changes.
			<br /><br />
			Please note that creating conflicts whilst inside edit mode is currently not supported. This feature
			is still experimental and may result in loss of work.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle id="editMode" checked={$editMode} on:click={() => ($editMode = !$editMode)} />
		</svelte:fragment>
	</SectionCard>
</ContentWrapper>

<style>
	.experimental-settings__text {
		color: var(--clr-text-2);
		margin-bottom: 12px;
	}
</style>
