<script lang="ts">
	import Formatter from "$lib/richText/plugins/Formatter.svelte";
	import FormattingButton from "$lib/richText/tools/FormattingButton.svelte";

	type Props = {
		formatter: ReturnType<typeof Formatter> | undefined;
	};

	let { formatter = $bindable() }: Props = $props();

	let scrollerElement = $state<HTMLDivElement | null>(null);
	let canScrollLeft = $state(false);
	let canScrollRight = $state(false);
	let hasScroll = $state(false);

	function updateScrollState() {
		if (!scrollerElement) return;

		const { scrollLeft, scrollWidth, clientWidth } = scrollerElement;
		hasScroll = scrollWidth > clientWidth;
		canScrollLeft = scrollLeft > 0;
		canScrollRight = scrollLeft < scrollWidth - clientWidth - 1; // -1 for rounding issues
	}

	$effect(() => {
		if (scrollerElement) {
			// Initial check
			updateScrollState();

			// Listen for scroll events
			function handleScroll() {
				updateScrollState();
			}
			scrollerElement.addEventListener("scroll", handleScroll);

			// Listen for resize events to update scroll state
			const resizeObserver = new ResizeObserver(updateScrollState);
			resizeObserver.observe(scrollerElement);

			return () => {
				scrollerElement?.removeEventListener("scroll", handleScroll);
				resizeObserver.disconnect();
			};
		}
	});
</script>

{#if formatter}
	<div class="formatting-bar">
		{#if hasScroll}
			<div class="curtain left" class:hidden={!canScrollLeft}></div>
		{/if}

		<div class="formatting-slides-scroller" bind:this={scrollerElement}>
			<div class="formatting-bar__group">
				<FormattingButton
					size="tag"
					icon="text-bold"
					activated={formatter.imports.isBold}
					tooltip="Bold"
					onclick={() => formatter.format("text-bold")}
				/>
				<FormattingButton
					size="tag"
					icon="text-italic"
					activated={formatter.imports.isItalic}
					tooltip="Italic"
					onclick={() => formatter.format("text-italic")}
				/>
				<FormattingButton
					size="tag"
					icon="text-strikethrough"
					activated={formatter.imports.isStrikethrough}
					tooltip="Strikethrough"
					onclick={() => formatter.format("text-strikethrough")}
				/>
			</div>
			<div class="formatting-bar__group">
				<FormattingButton
					size="tag"
					icon="text-code"
					activated={formatter.imports.isCode}
					tooltip="Code"
					onclick={() => formatter.format("text-code")}
				/>
				<FormattingButton
					size="tag"
					icon="text-quote"
					activated={formatter.imports.isQuote}
					tooltip="Quote"
					onclick={() => formatter.format("text-quote")}
				/>
				<FormattingButton
					size="tag"
					icon="text-link"
					activated={formatter.imports.isLink}
					tooltip="Link"
					onclick={() => formatter.format("text-link")}
				/>
			</div>
			<div class="formatting-bar__group">
				<FormattingButton
					size="tag"
					icon="text"
					activated={formatter?.imports.isNormal}
					tooltip="Normal text"
					onclick={() => formatter?.format("text")}
				/>
				<FormattingButton
					size="tag"
					icon="text-h2"
					activated={formatter?.imports.isH2}
					tooltip="Heading 2"
					onclick={() => formatter?.format("text-h2")}
				/>
				<FormattingButton
					size="tag"
					icon="text-h3"
					activated={formatter?.imports.isH3}
					tooltip="Heading 3"
					onclick={() => formatter?.format("text-h3")}
				/>
			</div>
			<div class="formatting-bar__group">
				<FormattingButton
					size="tag"
					icon="bullet-list"
					tooltip="Unordered list"
					onclick={() => formatter?.format("bullet-list")}
				/>
				<FormattingButton
					size="tag"
					icon="number-list"
					tooltip="Ordered list"
					onclick={() => formatter?.format("number-list")}
				/>
				<FormattingButton
					size="tag"
					icon="checklist"
					tooltip="Check list"
					onclick={() => formatter?.format("checklist")}
				/>
			</div>
		</div>

		{#if hasScroll}
			<div class="curtain right" class:hidden={!canScrollRight}></div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.formatting-bar {
		display: flex;
		position: relative;
		flex: 1;
		align-items: center;
		overflow: hidden;
	}

	.formatting-slides-scroller {
		display: flex;
		overflow-x: auto;
		overflow-y: hidden;
		scroll-snap-type: x mandatory;
		scrollbar-width: none; /* Firefox */
		-ms-overflow-style: none; /* IE/Edge */
	}

	.formatting-slides-scroller::-webkit-scrollbar {
		display: none; /* Chrome/Safari */
	}

	.formatting-bar__group {
		display: flex;
		align-items: center;
		gap: 2px;
		scroll-snap-align: start;

		&:not(:last-child) {
			&::after {
				display: block;
				width: 1px;
				height: 16px;
				margin: 0 6px;
				background-color: var(--clr-border-2);
				content: "";
			}
		}
	}

	.curtain {
		z-index: var(--z-ground);
		position: absolute;
		top: 0;
		bottom: 0;
		width: 20px;
		background: linear-gradient(
			to right,
			var(--clr-bg-2) 0%,
			oklch(from var(--clr-bg-2) l c h / 0) 100%
		);
		pointer-events: none;
		transition: opacity var(--transition-fast);

		&.left {
			left: 0;
		}
		&.right {
			right: 0;
			transform: rotateY(180deg);
		}
		&.hidden {
			opacity: 0;
		}
	}
</style>
