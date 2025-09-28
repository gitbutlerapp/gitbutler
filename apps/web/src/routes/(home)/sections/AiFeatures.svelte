<script lang="ts">
	import Features from '$home/components/Features.svelte';
	import contentJSON from '$home/data/content.json';

	const aiFetures = contentJSON['ai-features'];
</script>

<section class="ai-features">
	<h2 class="ai-features__title section-title"><i>Orchestrate Your</i> AI Tools ✨</h2>
	<div class="ai-features__video">
		<iframe
			width="100%"
			height="100%"
			src="https://www.youtube.com/embed/lTOo4v5KGkk?si=T7arPbqm9iJIzxZQ"
			title="YouTube video player"
			frameborder="0"
			allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
			referrerpolicy="strict-origin-when-cross-origin"
			allowfullscreen
		></iframe>
	</div>

	<Features items={aiFetures} />
</section>

<style>
	.ai-features {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
		border-radius: var(--radius-xl);
		background-color: var(--clr-bg-2);
	}

	.ai-features__title {
		grid-column: narrow-start / narrow-end;
		margin-bottom: 30px;
	}

	.ai-features__video {
		position: relative;
		grid-column: narrow-start / -3;
		aspect-ratio: 16 / 9;
		width: 100%;
		overflow: hidden;
		border-radius: var(--radius-xl);
		box-shadow: 0 4px 14px rgba(0, 0, 0, 0.1);
	}
</style>
