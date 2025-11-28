<script lang="ts">
	import Features from '$home/components/Features.svelte';
	import contentJSON from '$home/data/content.json';
	import { effectiveThemeStore } from '$lib/utils/theme.svelte';

	const previewContent = contentJSON['app-preview'];
	const featuresContent = contentJSON['main-features'];

	// Get the effective theme (light or dark) and reactive image source
	const previewSrc = $derived(
		$effectiveThemeStore === 'dark' ? previewContent['dark-src'] : previewContent['light-src']
	);
</script>

<div class="features-wrap">
	<img class="app-preview" src={previewSrc} alt={previewContent.alt} />

	<Features items={featuresContent} />
</div>

<style lang="postcss">
	.features-wrap {
		grid-column: full-start / full-end;
	}

	.app-preview {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-xl);
		box-shadow: 0 10px 20px rgba(0, 0, 0, 0.1);

		:global(.dark) & {
			box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4);
		}
	}
</style>
