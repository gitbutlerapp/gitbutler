<script lang="ts">
	import { pxToRem } from '$lib/utils/pxToRem';

	interface Props {
		hovered: boolean;
		activated: boolean;
		yOffsetPx?: number;
		isPreviewing?: boolean;
	}

	const { hovered, activated, yOffsetPx = 0, isPreviewing = false }: Props = $props();
</script>

<div
	class="dropzone-target container"
	class:activated
	class:hovered
	class:previewing={isPreviewing}
	style="--y-offset: {pxToRem(yOffsetPx) || 0}"
>
	<div class="indicator"></div>
</div>

<style lang="postcss">
	.container {
		--dropzone-height: 16px;
		--dropzone-overlap: calc(var(--dropzone-height) / 2);

		height: var(--dropzone-height);
		margin-top: calc(var(--dropzone-overlap) * -1);
		margin-bottom: calc(var(--dropzone-overlap) * -1);
		background-color: rgba(0, 0, 0, 0.3);
		width: 100%;
		position: relative;
		top: var(--y-offset);

		display: flex;
		align-items: center;

		z-index: var(--z-floating);

		/* It is very important that all children are pointer-events: none */
		/* https://stackoverflow.com/questions/7110353/html5-dragleave-fired-when-hovering-a-child-element */
		& * {
			pointer-events: none;
		}

		&:not(.activated) {
			display: none;
		}

		&.hovered {
			& .indicator {
				opacity: 1;
			}
		}
	}

	.previewing {
		background-color: red;
	}

	.indicator {
		width: 100%;
		height: 3px;
		transition: opacity 0.1s;
		background-color: var(--clr-border-2);
		opacity: 0;
	}
</style>
