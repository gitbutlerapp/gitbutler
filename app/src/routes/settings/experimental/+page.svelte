<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { featureBaseBranchSwitching } from '$lib/config/uiFeatureFlags';
	import ContentWrapper from '$lib/settings/ContentWrapper.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';

	const baseBranchSwitching = featureBaseBranchSwitching();
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
				on:click={() => ($baseBranchSwitching = !$baseBranchSwitching)}
			/>
		</svelte:fragment>
	</SectionCard>
</ContentWrapper>

<style>
	.experimental-settings__text {
		color: var(--clr-text-2);
		margin-bottom: 12px;
	}
</style>
