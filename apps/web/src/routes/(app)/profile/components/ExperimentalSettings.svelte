<script lang="ts">
	import { featureShowOrganizations, featureShowProjectPage } from "$lib/featureFlags";
	import { CardGroup, Spacer, Toggle } from "@gitbutler/ui";
</script>

<Spacer />

<div class="stack-v gap-8">
	<h2 class="text-15 text-bold">Experimentell</h2>
	<p class="text-12 text-body clr-text-2">
		Diese Einstellungen aktivieren experimentelle Funktionen, die sich noch in der Entwicklung befinden.
		<br />
		Sie können instabil oder unvollständig sein. Die Nutzung erfolgt auf eigenes Risiko.
	</p>
</div>

<CardGroup>
	<CardGroup.Item labelFor="showOrganizations">
		{#snippet title()}Organisationen{/snippet}
		{#snippet caption()}
			Organisationen sind eine Möglichkeit, Projekte miteinander zu verknüpfen.
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
		{#snippet title()}Benutzer- / Organisations- / Projektseiten{/snippet}
		{#snippet caption()}
			Dies zeigt die vorläufigen Landingpages für Organisationen, Benutzer und Projekte an.
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
