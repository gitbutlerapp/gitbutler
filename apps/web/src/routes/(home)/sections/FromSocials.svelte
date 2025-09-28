<script lang="ts">
	import ArrowButton from '$home/components/ArrowButton.svelte';
	import SectionHeader from '$home/components/SectionHeader.svelte';
</script>

<section class="from-socials">
	<SectionHeader>
		<i>Community</i> voices

		{#snippet buttons()}
			<ArrowButton reverseDirection onclick={() => {}} />
			<ArrowButton onclick={() => {}} />
		{/snippet}
	</SectionHeader>

	<div class="video-carousel"></div>
</section>

<style>
	.from-socials {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
	}
</style>
