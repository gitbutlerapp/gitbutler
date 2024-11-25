<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import {
		cloudFunctionality,
		cloudCommunicationFunctionality,
		cloudReviewFunctionality
	} from '$lib/config/uiFeatureFlags';
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
	import { User } from '$lib/stores/user';
	import { getContextStore } from '@gitbutler/shared/context';
	import Toggle from '@gitbutler/ui/Toggle.svelte';

	const user = getContextStore(User);

	function toggleCloudFunctionality() {
		if ($cloudFunctionality) {
			$cloudFunctionality = false;
			$cloudCommunicationFunctionality = false;
			$cloudReviewFunctionality = false;
		} else {
			$cloudFunctionality = true;
		}
	}
</script>

<SettingsPage title="Experimental features">
	<p class="experimental-settings__text">
		This section contains a list of feature flags for features that are still in development or in
	</p>

	{#if $user?.role === 'admin'}
		<div class="experimental-settings__toggles">
			<div>
				<SectionCard labelFor="cloudFunctionality" orientation="row" roundedBottom={false}>
					<svelte:fragment slot="title">Online functionality</svelte:fragment>
					<svelte:fragment slot="caption">
						Very experimental online functionality powered by the GitButler backend. Subject to lots
						of change. Data may get deleted as development of these features progresses, or the
						features might get dropped entirly.
					</svelte:fragment>
					<svelte:fragment slot="actions">
						<Toggle
							id="cloudFunctionality"
							checked={$cloudFunctionality}
							onclick={() => toggleCloudFunctionality()}
						/>
					</svelte:fragment>
				</SectionCard>
				<SectionCard
					labelFor="cloudCommunicationFunctionality"
					orientation="row"
					roundedTop={false}
					roundedBottom={false}
					disabled={!$cloudFunctionality}
				>
					<svelte:fragment slot="title">Social Coding</svelte:fragment>
					<svelte:fragment slot="caption">
						Highly experimental feature for collaborating around a project a community.
					</svelte:fragment>
					<svelte:fragment slot="actions">
						<Toggle
							id="cloudCommunicationFunctionality"
							checked={$cloudCommunicationFunctionality}
							onclick={() => ($cloudCommunicationFunctionality = !$cloudCommunicationFunctionality)}
							disabled={!$cloudFunctionality}
						/>
					</svelte:fragment>
				</SectionCard>
				<SectionCard
					labelFor="cloudReviewFunctionality"
					orientation="row"
					roundedTop={false}
					disabled={!$cloudFunctionality}
				>
					<svelte:fragment slot="title">Patch Review</svelte:fragment>
					<svelte:fragment slot="caption">
						Highly experimental feature for reviewing code in an interdiff style.
					</svelte:fragment>
					<svelte:fragment slot="actions">
						<Toggle
							id="cloudReviewFunctionality"
							checked={$cloudReviewFunctionality}
							onclick={() => ($cloudReviewFunctionality = !$cloudReviewFunctionality)}
							disabled={!$cloudFunctionality}
						/>
					</svelte:fragment>
				</SectionCard>
			</div>
		</div>
	{/if}
</SettingsPage>

<style>
	.experimental-settings__text {
		color: var(--clr-text-2);
		margin-bottom: 10px;
	}
</style>
