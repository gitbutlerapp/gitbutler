<script lang="ts">
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';

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

	const { visible, hovered, activated, label = 'Drop here', extraPaddings }: Props = $props();
	let defaultPadding = 4;

	const extraPaddingTop = extraPaddings?.top ?? 0;
	const extraPaddingRight = extraPaddings?.right ?? 0;
	const extraPaddingBottom = extraPaddings?.bottom ?? 0;
	const extraPaddingLeft = extraPaddings?.left ?? 0;
</script>

<div
	class="dropzone-target dropzone-wrapper"
	class:visible
	class:activated
	class:hovered
	style="--padding-top: {pxToRem(defaultPadding + extraPaddingTop)}rem; --padding-right: {pxToRem(
		defaultPadding + extraPaddingRight
	)}rem; --padding-bottom: {pxToRem(
		defaultPadding + extraPaddingBottom
	)}rem; --padding-left: {pxToRem(defaultPadding + extraPaddingLeft)}rem"
>
	<div class="container">
		{#if label !== ''}
			<div class="dropzone-label">
				<svg
					class="dropzone-label-icon"
					viewBox="0 0 15 14"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
				>
					<path
						d="M12.5303 7.73599L8.23738 3.4431C7.84686 3.05257 7.21369 3.05257 6.82317 3.4431L2.53027 7.73599"
						stroke="currentColor"
						stroke-width="1.5"
					/>
					<path d="M7.53027 3.73602L7.53027 11.736" stroke="currentColor" stroke-width="1.5" />
				</svg>

				<span class="text-11 text-bold">{label}</span>
			</div>
		{/if}

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

			.dropzone-label {
				transform: translateY(0) scale(1);
				opacity: 1;
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

	.dropzone-label {
		display: flex;
		z-index: 1;
		align-items: center;
		padding: 4px 8px 4px 6px;
		gap: 5px;
		transform: translateY(3px) scale(0.95);
		border-radius: 100px;
		background-color: var(--clr-theme-gray-element);
		color: var(--clr-theme-gray-on-element);
		opacity: 0;

		transition:
			opacity 0.1s,
			transform 0.15s;
		will-change: transform, opacity;
	}

	.dropzone-label-icon {
		width: 14px;
		height: 14px;
		animation: icon-shifting 1s infinite;
	}

	@keyframes icon-shifting {
		0% {
			transform: translateY(0);
		}
		50% {
			transform: translateY(-2px);
		}
		100% {
			transform: translateY(0);
		}
	}

	.animated-rectangle {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;

		& rect {
			fill: var(--dropzone-fill);
			stroke: var(--dropzone-stroke);
			stroke-width: 2px;
			stroke-dasharray: 2;
			stroke-dashoffset: 30;
			transform-origin: center;
			transition:
				fill var(--transition-fast),
				stroke var(--transition-fast);
		}
	}
</style>
