<script lang="ts">
	import { stringToColor } from '$lib/utils/stringToColor';
	import { tooltip } from '$lib/utils/tooltip';

	interface Props {
		srcUrl: string;
		tooltipText: string;
		altText: string;
		size?: 'small' | 'medium';
	}

	let isLoaded = $state(false);

	const { srcUrl, tooltipText, altText, size = 'small' }: Props = $props();
</script>

<div class="image-wrapper {size}" style:background-color={stringToColor(altText)}>
	<img
		class="avatar"
		alt={altText}
		src={srcUrl}
		loading="lazy"
		onload={() => (isLoaded = true)}
		class:show={isLoaded}
		use:tooltip={tooltipText}
	/>
</div>

<style lang="postcss">
	.image-wrapper {
		display: grid;
		place-content: center;
		overflow: hidden;
		border-radius: 6px;
		width: 12px;
		height: 12px;

		&.small {
			border-radius: 6px;
			width: 12px;
			height: 12px;
		}

		&.medium {
			border-radius: 8px;
			width: 16px;
			height: 16px;
		}
	}

	.image-wrapper > * {
		grid-area: 1 / 1;
	}

	.avatar {
		position: relative;
		width: 100%;
		height: 100%;
		object-fit: cover;
		opacity: 0;
	}

	.show {
		opacity: 1;
	}
</style>
