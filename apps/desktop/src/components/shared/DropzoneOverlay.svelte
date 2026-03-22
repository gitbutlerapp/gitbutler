<script lang="ts">
	import { injectOptional } from "@gitbutler/core/context";
	import { DRAG_STATE_SERVICE } from "@gitbutler/ui/drag/dragStateService.svelte";
	import { pxToRem } from "@gitbutler/ui/utils/pxToRem";

	interface Props {
		hovered: boolean;
		activated: boolean;
		visible?: boolean;
		label?: string;
		extraPaddings?: {
			top?: number;
			right?: number;
			bottom?: number;
			left?: number;
		};
	}

	const { visible, hovered, activated, label = "Drop here", extraPaddings }: Props = $props();
	let defaultPadding = 4;
	const dragStateService = injectOptional(DRAG_STATE_SERVICE, undefined);

	const extraPaddingTop = extraPaddings?.top ?? 0;
	const extraPaddingRight = extraPaddings?.right ?? 0;
	const extraPaddingBottom = extraPaddings?.bottom ?? 0;
	const extraPaddingLeft = extraPaddings?.left ?? 0;

	$effect(() => {
		if (activated && hovered && label && dragStateService) {
			// Set the label and get a token for cleanup
			const token = dragStateService.setDropLabel(label);
			return () => {
				dragStateService.clearDropLabel(token);
			};
		}
	});
</script>

<div
	class="dropzone-target dropzone-wrapper"
	class:visible
	class:activated
	class:hovered
	style="--padding-top: {pxToRem(defaultPadding + extraPaddingTop)}rem; --padding-right: {pxToRem(
		defaultPadding + extraPaddingRight,
	)}rem; --padding-bottom: {pxToRem(
		defaultPadding + extraPaddingBottom,
	)}rem; --padding-left: {pxToRem(defaultPadding + extraPaddingLeft)}rem"
>
	<div class="container">
		<!-- SVG rectangle to simulate a dashed outline with a precise dash offset. -->
		<svg width="100%" height="100%" class="animated-rectangle">
			<rect width="100%" height="100%" rx="6" ry="6" vector-effect="non-scaling-stroke" />
		</svg>
	</div>
</div>

<style lang="postcss">
	.dropzone-wrapper {
		display: none;
		z-index: var(--z-floating);
		position: absolute;
		top: 0;
		left: 0;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
		padding-top: var(--padding-top);
		padding-right: var(--padding-right);
		padding-bottom: var(--padding-bottom);
		padding-left: var(--padding-left);

		&.visible {
			display: flex;

			& .animated-rectangle rect {
				fill: transparent;
				stroke: var(--clr-border-2);
			}
		}

		&:not(.visible).activated {
			display: flex;
		}

		&.visible.activated {
			& .animated-rectangle rect {
				fill: var(--dropzone-fill);
				stroke: var(--dropzone-stroke);
			}
		}

		&.hovered {
			.animated-rectangle rect {
				fill: var(--dropzone-fill-hover);
				stroke: var(--dropzone-stroke-hover);
				animation: dropzone-dash 4s linear infinite;
			}
		}
	}

	.container {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
		pointer-events: none;
	}
	.animated-rectangle {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;

		& rect {
			transform-origin: center;
			fill: var(--dropzone-fill);
			stroke: var(--dropzone-stroke);
			stroke-dasharray: 2;
			stroke-dashoffset: 30;
			stroke-width: 2px;
			transition:
				fill var(--transition-fast),
				stroke var(--transition-fast);
		}
	}
</style>
