<script lang="ts">
	import SettingsPage from '$components/SettingsPage.svelte';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import {
		cloudFunctionality,
		cloudCommunicationFunctionality,
		cloudReviewFunctionality
	} from '$lib/config/uiFeatureFlags';
	import { User } from '$lib/user/user';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';

	const settingsService = getContext(SettingsService);
	const settingsStore = settingsService.appSettings;

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
