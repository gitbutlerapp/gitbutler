<script lang="ts">
	import Button from '$lib/Button.svelte';
	import Formatter from '$lib/richText/plugins/Formatter.svelte';
	import FormattingButton from '$lib/richText/tools/FormattingButton.svelte';

	type Props = {
		formatter: ReturnType<typeof Formatter> | undefined;
	};

	let { formatter = $bindable() }: Props = $props();

	let slideWidth = $state(0);
	let isSecondSlide = $state(false);
</script>

<div class="formatting-bar">
	{#if formatter}
		<div class="formatting-slides" style:width="calc({slideWidth}px + 0.5rem)">
			<div class="formatting-slides-scroller" class:formatting-slides--second-slide={isSecondSlide}>
				<div class="formatting-bar__slide" bind:clientWidth={slideWidth}>
					<div class="formatting-bar__group">
						<FormattingButton
							icon="text-bold"
							activated={formatter.imports.isBold}
							tooltip="Bold"
							onclick={() => formatter.format('text-bold')}
						/>
						<FormattingButton
							icon="text-italic"
							activated={formatter.imports.isItalic}
							tooltip="Italic"
							onclick={() => formatter.format('text-italic')}
						/>
						<FormattingButton
							icon="text-strikethrough"
							activated={formatter?.imports.isStrikethrough}
							tooltip="Strikethrough"
							onclick={() => formatter?.format('text-strikethrough')}
						/>
					</div>
					<div class="formatting-bar__group">
						<FormattingButton
							icon="text-code"
							activated={formatter?.imports.isCode}
							tooltip="Code"
							onclick={() => formatter?.format('text-code')}
						/>
						<FormattingButton
							icon="text-quote"
							activated={formatter?.imports.isQuote}
							tooltip="Quote"
							onclick={() => formatter?.format('text-quote')}
						/>
						<FormattingButton
							icon="text-link"
							activated={formatter?.imports.isLink}
							tooltip="Link"
							onclick={() => formatter?.format('text-link')}
						/>
					</div>
				</div>

				<div class="formatting-bar__slide">
					<div class="formatting-bar__group">
						<FormattingButton
							icon="text"
							activated={formatter?.imports.isNormal}
							tooltip="Normal text"
							onclick={() => formatter?.format('text')}
						/>
						<FormattingButton
							icon="text-h2"
							activated={formatter?.imports.isH2}
							tooltip="Heading 2"
							onclick={() => formatter?.format('text-h2')}
						/>
						<FormattingButton
							icon="text-h3"
							activated={formatter?.imports.isH3}
							tooltip="Heading 3"
							onclick={() => formatter?.format('text-h3')}
						/>
					</div>
					<div class="formatting-bar__group">
						<FormattingButton
							icon="bullet-list"
							tooltip="Unordered list"
							onclick={() => formatter?.format('bullet-list')}
						/>
						<FormattingButton
							icon="number-list"
							tooltip="Ordered list"
							onclick={() => formatter?.format('number-list')}
						/>
						<FormattingButton
							icon="checklist"
							tooltip="Check list"
							onclick={() => formatter?.format('checklist')}
						/>
					</div>
				</div>
			</div>
		</div>

		<Button
			style="neutral"
			kind="ghost"
			icon={isSecondSlide ? 'chevron-left' : 'chevron-right'}
			tooltip="More options"
			tooltipDelay={500}
			tooltipPosition="bottom"
			onclick={() => {
				isSecondSlide = !isSecondSlide;
			}}
		/>
	{/if}
</div>

<style lang="postcss">
	.formatting-bar {
		display: flex;
		align-items: center;
		width: fit-content;
	}

	.formatting-slides {
		position: relative;
		display: flex;
		overflow: hidden;
		margin-right: 6px;
		padding-left: 8px;
		margin-left: -8px;
		/* background-color: antiquewhite; */

		&::after {
			user-select: none;
			content: '';
			position: absolute;
			top: 0;
			left: 0;
			width: 8px;
			height: 100%;
			background: linear-gradient(
				to right,
				var(--clr-bg-1) 0%,
				oklch(from var(--clr-bg-1) l c h / 0) 100%
			);
		}
	}

	.formatting-slides-scroller {
		display: flex;
		transition: transform 0.2s ease;
	}

	.formatting-bar__slide {
		display: flex;
		align-items: center;
	}

	.formatting-bar__group {
		display: flex;
		align-items: center;

		&:not(:last-child) {
			&::after {
				content: '';
				display: block;
				width: 1px;
				height: 16px;
				background-color: var(--clr-border-3);
				margin: 0 6px;
			}
		}
	}

	.formatting-slides--second-slide {
		transform: translateX(-50%);
	}
</style>
