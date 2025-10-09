<script lang="ts">
	import Features from '$home/components/Features.svelte';
	import SectionHeader from '$home/components/SectionHeader.svelte';
	import contentJSON from '$home/data/content.json';

	const aiFetures = contentJSON['ai-features'];
</script>

<section class="ai-features">
	<SectionHeader>
		<i>Orchestrate</i> your AI Tools ✨
	</SectionHeader>
	<div class="ai-features__video">
		<iframe
			width="100%"
			height="100%"
			src={contentJSON['ai-fetures-demo']}
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

	.ai-features__video {
		position: relative;
		grid-column: narrow-start / narrow-end;
		aspect-ratio: 16 / 9;
		width: 100%;
		overflow: hidden;
		border-radius: var(--radius-xl);
		box-shadow: 0 4px 14px rgba(0, 0, 0, 0.1);
	}
</style>
