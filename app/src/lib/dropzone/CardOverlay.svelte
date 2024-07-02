<script lang="ts">
	import Icon from '$lib/shared/Icon.svelte';

	interface Props {
		hovered: boolean;
		activated: boolean;
		label?: string;
	}

	const { hovered, activated, label = 'Drop here' }: Props = $props();
</script>

<div class="dropzone-target container" class:activated class:hovered>
	<div class="dropzone-content">
		<Icon name="new-file-small-filled" />
		<p class="text-base-13">{label}</p>
	</div>
</div>

<style lang="postcss">
	:root {
		--dropzone-height: 16px;
		--dropzone-overlap: calc(var(--dropzone-height) / 2);
	}

	.container {
		position: absolute;
		width: 100%;
		height: 100%;

		display: flex;
		align-items: center;
		justify-content: center;

		background-color: oklch(from var(--clr-scale-pop-70) l c h / 0.1);

		outline-color: var(--clr-scale-pop-40);
		outline-style: dashed;
		outline-width: 1px;
		outline-offset: -10px;
		backdrop-filter: blur(10px);

		z-index: var(--z-lifted);

		transition: background-color 0.1s;

		/* It is very important that all children are pointer-events: none */
		/* https://stackoverflow.com/questions/7110353/html5-dragleave-fired-when-hovering-a-child-element */
		& * {
			pointer-events: none;
		}

		&:not(.activated) {
			display: none;
		}

		&.hovered {
			background-color: oklch(from var(--clr-scale-pop-20) l c h / 0.1);
		}
	}

	.dropzone-content {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--clr-scale-pop-40);
	}
</style>
