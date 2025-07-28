<script lang="ts">
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';

	interface Props {
		hovered: boolean;
		activated: boolean;
		yOffsetPx?: number;
	}

	const { hovered, activated, yOffsetPx = 0 }: Props = $props();
</script>

<div
	class="dropzone-target container"
	class:activated
	class:hovered
	style:--y-offset="{pxToRem(yOffsetPx)}rem"
>
	<div class="indicator"></div>
</div>

<style lang="postcss">
	.container {
		--dropzone-overlap: calc(var(--dropzone-height) / 2);
		--dropzone-height: 24px;

		display: flex;

		z-index: var(--z-floating);

		position: absolute;
		top: var(--y-offset);
		align-items: center;
		width: 100%;

		height: var(--dropzone-height);
		margin-top: calc(var(--dropzone-overlap) * -1);

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
				background-color: var(--clr-theme-pop-element);
			}
		}
	}

	.indicator {
		width: 100%;
		height: 3px;
		margin-top: 1px;
		background-color: transparent;
		transition: opacity 0.1s;
	}
</style>
