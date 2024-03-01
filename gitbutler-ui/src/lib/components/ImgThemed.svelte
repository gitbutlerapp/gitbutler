<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	export let imgSet: { light: string; dark: string };

	let imgSrc = '';

	const updateImage = () => {
		const colorScheme = getComputedStyle(document.documentElement)
			.getPropertyValue('color-scheme')
			.trim();
		imgSrc = colorScheme === 'dark' ? imgSet.dark : imgSet.light;
	};

	let observer: MutationObserver;

	onMount(() => {
		updateImage();
		observer = new MutationObserver(updateImage);
		observer.observe(document.documentElement, {
			attributes: true,
			attributeFilter: ['style']
		});
	});

	onDestroy(() => {
		observer.disconnect();
	});
</script>

<img src={imgSrc} alt="Decorative Art" class="themed-image" />

<style>
	.themed-image {
		display: inline-block;
		-webkit-user-drag: none;
		user-select: none;
	}
</style>
