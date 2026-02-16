<script lang="ts">
	import featuresJson from "../features.json";
	import Features from "$home/components/Features.svelte";
	import SectionHeader from "$home/components/SectionHeader.svelte";
</script>

<section class="cli-features">
	<SectionHeader>
		Everything <i>You Wished</i> Git Had
	</SectionHeader>
	<Features items={featuresJson} noMargin />
</section>

<style>
	.cli-features {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
		margin-top: 46px;
	}

	@media (max-width: 700px) {
		.cli-features {
			margin-top: 0;
		}
	}
</style>
