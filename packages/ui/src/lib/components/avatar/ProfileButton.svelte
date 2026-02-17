<script lang="ts">
	import profileIconSvg from "$lib/assets/profile-icon.svg?raw";
	import { useImageLoading } from "$lib/utils/imageLoading.svelte";

	interface Props {
		onclick: () => void;
		srcUrl?: string | null;
	}

	const { onclick, srcUrl }: Props = $props();

	const placeholderUrl = `data:image/svg+xml;utf8,${encodeURIComponent(profileIconSvg)}`;
	const imageLoadingState = useImageLoading();
</script>

<button
	type="button"
	class="profile-btn"
	class:has-image={!!srcUrl}
	aria-label="Profile button"
	onclick={async () => onclick()}
>
	{#if srcUrl && !imageLoadingState.hasError}
		<img
			bind:this={imageLoadingState.imgElement}
			src={srcUrl}
			alt="Profile"
			class="hidden-preload"
			referrerpolicy="no-referrer"
			onload={imageLoadingState.handleImageLoad}
			onerror={imageLoadingState.handleImageError}
		/>
	{/if}
	<div
		class="profile-image"
		class:loaded={imageLoadingState.imageLoaded || !srcUrl || imageLoadingState.hasError}
		style:background-image={srcUrl && !imageLoadingState.hasError
			? `url(${srcUrl})`
			: `url("${placeholderUrl}")`}
	></div>
</button>

<style lang="postcss">
	.profile-btn {
		display: flex;
		align-items: center;
		aspect-ratio: 1 / 1;
		width: 34px;
		overflow: hidden;
		border-radius: 50%;
		background: var(--clr-theme-pop-element);
		cursor: pointer;
		transition: background-color var(--transition-medium);

		&:not(.has-image):hover {
			background-color: var(--hover-pop);
		}

		&.has-image:hover {
			background-color: var(--hover-pop);

			.profile-image {
				filter: brightness(0.9);
			}
		}
	}

	.profile-image {
		width: 100%;
		height: 100%;
		border-radius: 50%;
		background-position: center;
		background-size: cover;
		opacity: 0;
		transition:
			filter var(--transition-medium),
			opacity 0.2s ease-in;

		&.loaded {
			opacity: 1;
		}
	}

	.hidden-preload {
		position: absolute;
		width: 1px;
		height: 1px;
		opacity: 0;
		pointer-events: none;
	}
</style>
