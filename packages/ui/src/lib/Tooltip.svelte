<script lang="ts" context="module">
	import TooltipWrapper, {
		type TooltipAlign,
		type TooltipPosition
	} from './shared/TooltipWrapper.svelte';

	export { type TooltipAlign, type TooltipPosition };
</script>

<script lang="ts">
	import { type Snippet } from 'svelte';

	interface Props {
		text?: string;
		delay?: number;
		align?: TooltipAlign;
		position?: TooltipPosition;
		children: Snippet;
	}

	const { text, delay, align, position, children }: Props = $props();
	const isTextEmpty = $derived(!text);
</script>

{#snippet tooltip()}
	<span class="tooltip">{text}</span>
{/snippet}

<TooltipWrapper {delay} {align} {position} {tooltip} disable={isTextEmpty}>
	{@render children()}
</TooltipWrapper>

<style lang="postcss">
	.tooltip {
		white-space: pre-line;
		display: flex;
		justify-content: center;
		flex-direction: column;
		pointer-events: none;
		background-color: var(--clr-tooltip-bg);
		border: 1px solid var(--clr-tooltip-border);
		border-radius: var(--radius-m);
		color: var(--clr-core-ntrl-80);
		display: inline-block;
		width: fit-content;
		max-width: 240px;
		padding: 4px 8px;
		text-align: left;
		box-shadow: var(--fx-shadow-s);
	}
</style>
