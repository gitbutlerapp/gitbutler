<script lang="ts">
	import profileIconSvg from '$lib/assets/profile-icon.svg?raw';

	interface Props {
		onclick: () => void;
		srcUrl?: string | null;
	}

	const { onclick, srcUrl }: Props = $props();

	const placeholderUrl = `data:image/svg+xml;utf8,${encodeURIComponent(profileIconSvg)}`;
</script>

<button
	type="button"
	class="profile-btn"
	class:has-image={!!srcUrl}
	aria-label="Profile button"
	onclick={async () => onclick()}
>
	<div
		class="profile-image"
		style:background-image={srcUrl ? `url(${srcUrl})` : `url("${placeholderUrl}")`}
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
		transition: filter var(--transition-medium);
	}
</style>
