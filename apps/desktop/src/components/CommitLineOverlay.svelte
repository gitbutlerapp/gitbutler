<script lang="ts">
	import { portal } from '@gitbutler/ui/utils/portal';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';

	interface Props {
		hovered: boolean;
		activated: boolean;
		advertize?: boolean;
		yOffsetPx?: number;
	}

	const { hovered, activated, advertize, yOffsetPx = 0 }: Props = $props();

	let containerElement = $state<HTMLDivElement>();
	let indicatorElement = $state<HTMLDivElement>();
	let indicatorRect = $state<{ top: number; left: number; width: number; height: number }>();
	let stackViewElement = $state<Element | null>(null);

	function updatePosition() {
		if (!indicatorElement) return;

		const rect = indicatorElement.getBoundingClientRect();
		const stackView = indicatorElement.closest('.stack-view');
		if (!stackView) return;

		stackViewElement = stackView;

		const stackRect = stackView.getBoundingClientRect();
		const scrollTop = stackView.scrollTop || 0;

		// Calculate position relative to .stack-view container including scroll offset
		indicatorRect = {
			top: rect.top - stackRect.top + scrollTop,
			left: rect.left - stackRect.left,
			width: rect.width,
			height: rect.height
		};
	}

	$effect(() => {
		if (containerElement && indicatorElement && activated) {
			updatePosition();
		}
	});
</script>

<div
	bind:this={containerElement}
	class="dropzone-target container"
	class:activated
	class:advertize
	class:hovered
	style:--y-offset="{pxToRem(yOffsetPx)}rem"
>
	<div bind:this={indicatorElement} class="indicator-placeholder"></div>
</div>

{#if activated && indicatorRect && stackViewElement}
	<div
		class="indicator-portal"
		class:hovered
		class:advertize
		use:portal={stackViewElement}
		style:top="{indicatorRect.top}px"
		style:left="{indicatorRect.left}px"
		style:width="{indicatorRect.width}px"
		style:height="{indicatorRect.height}px"
	>
		<div class="indicator"></div>
	</div>
{/if}

<style lang="postcss">
	.container {
		--dropzone-overlap: calc(var(--dropzone-height) / -2);
		--dropzone-height: 24px;

		display: flex;
		z-index: var(--z-modal);
		position: absolute;
		top: var(--y-offset);
		align-items: center;
		width: 100%;
		height: var(--dropzone-height);
		margin-top: var(--dropzone-overlap);
		/* For debugging  */
		/* background-color: rgba(238, 130, 238, 0.319); */

		&:not(.activated) {
			display: none;
		}

		& > * {
			pointer-events: none; /* Block all nested elements */
		}
	}

	.indicator-placeholder {
		width: 100%;
		height: 3px;
		margin-top: 1px;
		background-color: transparent;
	}

	.indicator-portal {
		display: flex;
		z-index: var(--z-blocker);
		position: absolute;
		align-items: center;
		pointer-events: none;

		&.hovered {
			& .indicator {
				background-color: var(--clr-theme-pop-element);
			}
		}
	}

	.indicator {
		width: 100%;
		height: 3px;
		background-color: transparent;
		transition: background-color var(--transition-fast);
	}
</style>
