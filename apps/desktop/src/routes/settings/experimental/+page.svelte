<script lang="ts">
	import SettingsPage from '$components/SettingsPage.svelte';
	import {
		cloudFunctionality,
		cloudCommunicationFunctionality,
		cloudReviewFunctionality,
		v3
	} from '$lib/config/uiFeatureFlags';
	import { User } from '$lib/user/user';
	import { getContextStore } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
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
					{#snippet title()}Online functionality{/snippet}
					{#snippet caption()}
						Very experimental online functionality powered by the GitButler backend. Subject to lots
						of change. Data may get deleted as development of these features progresses, or the
						features might get dropped entirly.
					{/snippet}
					{#snippet actions()}
						<Toggle
							id="cloudFunctionality"
							checked={$cloudFunctionality}
							onclick={() => toggleCloudFunctionality()}
						/>
					{/snippet}
				</SectionCard>
				<SectionCard
					labelFor="cloudCommunicationFunctionality"
					orientation="row"
					roundedTop={false}
					roundedBottom={false}
					disabled={!$cloudFunctionality}
				>
					{#snippet title()}Social Coding{/snippet}
					{#snippet caption()}
						Highly experimental feature for collaborating around a project a community.
					{/snippet}
					{#snippet actions()}
						<Toggle
							id="cloudCommunicationFunctionality"
							checked={$cloudCommunicationFunctionality}
							onclick={() => ($cloudCommunicationFunctionality = !$cloudCommunicationFunctionality)}
							disabled={!$cloudFunctionality}
						/>
					{/snippet}
				</SectionCard>
				<SectionCard
					labelFor="cloudReviewFunctionality"
					orientation="row"
					roundedTop={false}
					disabled={!$cloudFunctionality}
				>
					{#snippet title()}Patch Review{/snippet}
					{#snippet caption()}
						Highly experimental feature for reviewing code in an interdiff style.
					{/snippet}
					{#snippet actions()}
						<Toggle
							id="cloudReviewFunctionality"
							checked={$cloudReviewFunctionality}
							onclick={() => ($cloudReviewFunctionality = !$cloudReviewFunctionality)}
							disabled={!$cloudFunctionality}
						/>
					{/snippet}
				</SectionCard>
			</div>

			<SectionCard orientation="row" centerAlign>
				{#snippet title()}
					V3 Design
				{/snippet}
				{#snippet caption()}
					The number of lines that trigger a snapshot when saving.
				{/snippet}

				{#snippet actions()}
					<Toggle id="v3Design" checked={$v3} onclick={() => ($v3 = !$v3)} />
				{/snippet}
			</SectionCard>
		</div>
	{/if}
</SettingsPage>

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
