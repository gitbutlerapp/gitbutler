<script lang="ts">
	type Props = {
		lightSrc: string;
		darkSrc: string;
		alt: string;
		width?: string;
		minWidth?: string;
		height?: string;
		class?: string;
		decorative?: boolean;
	};

	const {
		lightSrc,
		darkSrc,
		alt,
		width,
		minWidth,
		height,
		class: className = "",
		decorative = false,
	}: Props = $props();
</script>

<span class="themed-image {className}" style:width style:min-width={minWidth} style:height>
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
		flex: none;
		align-items: center;
		justify-content: center;
		width: fit-content;
		max-width: 100%;
	}

	.themed-image__asset {
		display: block;
		flex: none;
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
