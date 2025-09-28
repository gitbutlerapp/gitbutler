<script lang="ts">
	import ArrowButton from '$home/components/ArrowButton.svelte';
	import SectionHeader from '$home/components/SectionHeader.svelte';
</script>

<section class="feature-updates">
	<SectionHeader>
		<i>Feature</i> updates

		{#snippet buttons()}
			<ArrowButton
				label="All demos"
				onclick={() =>
					window.open(
						'https://youtube.com/playlist?list=PLNXkW_le40U7IH8qA5VPN6f01oC25LOj4&si=AslJQNwX6M-JP_wR',
						'_blank'
					)}
			/>
		{/snippet}
	</SectionHeader>

	<div class="video-carousel"></div>
</section>

<style>
	.feature-updates {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
	}

	.feature-updates__header {
		display: flex;
		grid-column: narrow-start / narrow-end;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 30px;
	}
</style>
