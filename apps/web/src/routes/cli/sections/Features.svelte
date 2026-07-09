<script lang="ts">
	import featuresJson from "../features.json";
	import Features from "$home/components/Features.svelte";
	import SectionHeader from "$home/components/SectionHeader.svelte";
</script>

<section class="cli-features">
	<SectionHeader>
		Alles, <i>was du dir</i> von Git gewünscht hast
	</SectionHeader>
	<Features items={featuresJson} noMargin />
</section>

<style>
	.cli-features {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
	}
</style>
