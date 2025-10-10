<script lang="ts">
	import { featureShowOrganizations, featureShowProjectPage } from '$lib/featureFlags';
	import { SectionCard, Toggle, Spacer } from '@gitbutler/ui';
</script>

<Spacer />

<div class="stack-v gap-8">
	<h2 class="text-15 text-bold">Experimental</h2>
	<p class="text-12 text-body clr-text-2">
		These settings enable experimental features that are still in development.
		<br />
		They may be unstable or incomplete. Use them at your own risk.
	</p>
</div>

<div class="stack-v">
	<SectionCard labelFor="showOrganizations" orientation="row" roundedBottom={false}>
		{#snippet title()}Organizations{/snippet}
		{#snippet caption()}
			Organizations are a way of linking together projects.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="showOrganizations"
				checked={$featureShowOrganizations}
				onclick={() => ($featureShowOrganizations = !$featureShowOrganizations)}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard labelFor="showProjectPage" orientation="row" roundedTop={false}>
		{#snippet title()}User / Organization / Project Pages{/snippet}
		{#snippet caption()}
			This will show the stub landing pages for orgs, users and projects.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="showProjectPage"
				checked={$featureShowProjectPage}
				onclick={() => ($featureShowProjectPage = !$featureShowProjectPage)}
			/>
		{/snippet}
	</SectionCard>
</div>

<style lang="postcss">
	.section-title {
		margin-top: 24px;
	}
</style>
