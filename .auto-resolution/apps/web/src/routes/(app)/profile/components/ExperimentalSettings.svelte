<script lang="ts">
	import { featureShowOrganizations, featureShowProjectPage } from '$lib/featureFlags';
	import { CardGroup, Spacer, Toggle } from '@gitbutler/ui';
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

<CardGroup>
	<CardGroup.Item labelFor="showOrganizations">
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
	</CardGroup.Item>

	<CardGroup.Item labelFor="showProjectPage">
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
	</CardGroup.Item>
</CardGroup>
