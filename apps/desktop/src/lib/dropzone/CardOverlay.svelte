<script lang="ts">
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';

	interface Props {
		hovered: boolean;
		activated: boolean;
		label?: string;
		extraPaddings?: {
			top?: number;
			right?: number;
			bottom?: number;
			left?: number;
		};
	}

	const { hovered, activated, label = 'Drop here', extraPaddings }: Props = $props();
	let defaultPadding = 4;

	const extraPaddingTop = extraPaddings?.top ?? 0;
	const extraPaddingRight = extraPaddings?.right ?? 0;
	const extraPaddingBottom = extraPaddings?.bottom ?? 0;
	const extraPaddingLeft = extraPaddings?.left ?? 0;
</script>

<div
	class="dropzone-target dropzone-wrapper"
	class:activated
	class:hovered
	style="--padding-top: {pxToRem(defaultPadding + extraPaddingTop)}; --padding-right: {pxToRem(
		defaultPadding + extraPaddingRight
	)}; --padding-bottom: {pxToRem(defaultPadding + extraPaddingBottom)}; --padding-left: {pxToRem(
		defaultPadding + extraPaddingLeft
	)}"
>
	<div class="container">
		<div class="dropzone-label">
			<svg
				class="dropzone-label-icon"
				viewBox="0 0 12 12"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path d="M11 7L6 2M6 2L1 7M6 2L6 12" stroke="white" stroke-width="1.5" />
			</svg>

			<span class="text-12 text-semibold">{label}</span>
		</div>

		<!-- add svg rectange -->
		<svg width="100%" height="100%" class="animated-rectangle">
			<rect width="100%" height="100%" rx="5" ry="5" vector-effect="non-scaling-stroke" />
		</svg>
	</div>
</div>

<style lang="postcss">
	.dropzone-wrapper {
		z-index: var(--z-ground);
		position: absolute;
		width: 100%;
		height: 100%;
		top: 0;
		left: 0;
		padding-top: var(--padding-top);
		padding-right: var(--padding-right);
		padding-bottom: var(--padding-bottom);
		padding-left: var(--padding-left);

		display: none;
		align-items: center;
		justify-content: center;

		transition:
			transform 0.1s,
			padding 0.1s;

		/* It is very important that all children are pointer-events: none */
		/* https://stackoverflow.com/questions/7110353/html5-dragleave-fired-when-hovering-a-child-element */
		& * {
			pointer-events: none;
		}

		&.activated {
			display: flex;
			animation: dropzone-scale 0.1s forwards;
		}

		&.hovered {
			transform: scale(1.01);

			.animated-rectangle rect {
				fill: oklch(from var(--clr-scale-pop-50) l c h / 0.16);
			}

			.dropzone-label {
				opacity: 1;
				transform: translateY(0) scale(1);
			}
		}
	}

	.container {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
	}

	.dropzone-label {
		opacity: 0;
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 10px;
		border-radius: 100px;
		color: var(--clr-theme-pop-on-element);
		background-color: var(--clr-theme-pop-element);
		transform: translateY(3px) scale(0.95);

		transition:
			opacity 0.1s,
			transform 0.15s;
	}

	.dropzone-label-icon {
		width: 12px;
		height: 12px;
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
			fill: oklch(from var(--clr-scale-pop-50) l c h / 0.1);
			stroke: var(--clr-scale-pop-50);

			stroke-width: 2px;
			stroke-dasharray: 2;
			stroke-dashoffset: 30;
			transform-origin: center;

			transition:
				fill var(--transition-fast),
				transform var(--transition-fast);

			animation: dash 4s linear infinite;
		}
	}

	@keyframes dash {
		from {
			stroke-dashoffset: 30;
		}
		to {
			stroke-dashoffset: 0;
		}
	}
</style>
