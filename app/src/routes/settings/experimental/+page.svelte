<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Toggle from '$lib/components/Toggle.svelte';
	import ContentWrapper from '$lib/components/settings/ContentWrapper.svelte';
	import {
		featureBaseBranchSwitching,
		featureAdvancedCommitOperations
	} from '$lib/config/uiFeatureFlags';
	const baseBranchSwitching = featureBaseBranchSwitching();
	const advancedCommitOperations = featureAdvancedCommitOperations();
</script>

<ContentWrapper title="Experimental features">
	<p class="text-base-body-13 experimental-settings__text">
		This sections contains a list of feature flags for features that are still in development or in
		an experimental stage.
	</p>
	<SectionCard labelFor="baseBranchSwitching" orientation="row">
		<svelte:fragment slot="title">Switching the target branch</svelte:fragment>
		<svelte:fragment slot="caption">
			This allows changing of the target branch (trunk) after the initial project setup from within
			the project settings. Not fully tested yet, use with caution.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="baseBranchSwitching"
				checked={$baseBranchSwitching}
				on:change={() => ($baseBranchSwitching = !$baseBranchSwitching)}
			/>
		</svelte:fragment>
	</SectionCard>
	<SectionCard labelFor="advancedCommitOperations" orientation="row">
		<svelte:fragment slot="title">Advanced commit operations</svelte:fragment>
		<svelte:fragment slot="caption">
			Allows for reordeing of commits, changing the message as well as undoing of commits anywhere
			in the stack. In addition it allows for adding an empty commit between two other commits.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="advancedCommitOperations"
				checked={$advancedCommitOperations}
				on:change={() => ($advancedCommitOperations = !$advancedCommitOperations)}
			/>
		</svelte:fragment>
	</SectionCard>
</ContentWrapper>

<style>
	.experimental-settings__text {
		color: var(--clr-text-2);
		margin-bottom: var(--size-12);
	}
</style>
