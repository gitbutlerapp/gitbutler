<script lang="ts">
	import Features from '$home/components/Features.svelte';
	import contentJSON from '$home/data/content.json';

	const previewContent = contentJSON['app-preview'];
	const featuresContent = contentJSON['main-features'];
</script>

<div class="features-wrap">
	<img class="app-preview" src={previewContent.src} alt={previewContent.alt} />

	<Features items={featuresContent} />
</div>

<style lang="scss">
	.features-wrap {
		grid-column: full-start / full-end;
	}

	.app-preview {
		filter: drop-shadow(0 10px 20px rgba(0, 0, 0, 0.1));
	}
</style>
