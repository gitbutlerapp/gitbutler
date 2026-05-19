<script lang="ts">
	type Props = {
		lightSrc: string;
		darkSrc: string;
		alt: string;
		width?: string;
		height?: string;
		class?: string;
		decorative?: boolean;
	};

	const {
		lightSrc,
		darkSrc,
		alt,
		width,
		height,
		class: className = "",
		decorative = false,
	}: Props = $props();
</script>

<span class="themed-image {className}" style:width style:height>
	<img
		class="themed-image__asset themed-image__asset--light"
		src={lightSrc}
		alt={decorative ? "" : alt}
		aria-hidden={decorative}
	/>
	<img
		class="themed-image__asset themed-image__asset--dark"
		src={darkSrc}
		alt={decorative ? "" : alt}
		aria-hidden={decorative}
	/>
</span>

<style lang="postcss">
	.themed-image {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: fit-content;
		max-width: 100%;
	}

	.themed-image__asset {
		display: block;
		width: auto;
		max-width: 100%;
		height: auto;
		max-height: 100%;
		object-fit: contain;
	}

	.themed-image__asset--dark {
		display: none;
	}

	:global(:root.dark) .themed-image__asset--light {
		display: none;
	}

	:global(:root.dark) .themed-image__asset--dark {
		display: block;
	}
</style>
