<script lang="ts">
	import { pxToRem } from '$lib/utils/pxToRem';

	interface Props {
		hovered: boolean;
		activated: boolean;
		yOffsetPx?: number;
	}

	const { hovered, activated, yOffsetPx = 0 }: Props = $props();

	console.log(yOffsetPx, pxToRem(yOffsetPx));
</script>

<div
	class="dropzone-target container"
	class:activated
	style="--y-offset: {pxToRem(yOffsetPx) || 0}"
>
	<div class="indicator" class:hovered></div>
</div>

<style lang="postcss">
	:root {
		--dropzone-height: 16px;
		--dropzone-overlap: calc(var(--dropzone-height) / 2);
	}

	.container {
		height: var(--dropzone-height);
		margin-top: calc(var(--dropzone-overlap) * -1);
		margin-bottom: calc(var(--dropzone-overlap) * -1);
		width: 100%;
		position: relative;
		top: var(--y-offset);

		display: flex;
		align-items: center;

		z-index: 101;

		/* It is very important that all children are pointer-events: none */
		/* https://stackoverflow.com/questions/7110353/html5-dragleave-fired-when-hovering-a-child-element */
		& * {
			pointer-events: none;
		}

		&:not(.activated) {
			display: none;
		}
	}

	.indicator {
		width: 100%;
		height: 3px;
		transition: background-color 0.1s;

		&.hovered {
			background-color: rgba(0, 0, 0, 0.6);
		}
	}
</style>
