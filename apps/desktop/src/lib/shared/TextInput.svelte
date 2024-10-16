<script lang="ts">
	import { resizeObserver } from '@gitbutler/ui/utils/resizeObserver';
	import type { HTMLAttributes } from 'svelte/elements';
	interface Props extends HTMLAttributes<HTMLTextAreaElement | HTMLInputElement> {
		value: string;
		multiline: boolean;
		disabled: boolean;
		element: HTMLTextAreaElement | HTMLInputElement | undefined;
		inputHeight: string;
		inputWidth: string;
		textAreaWidth: string;
		autocomplete: string;
	}
	let {
		multiline,
		value = $bindable(),
		inputHeight = $bindable(),
		inputWidth = $bindable(),
		element = $bindable(),
		textAreaWidth = $bindable(),
		...rest
	}: Props = $props();
</script>

{#if multiline}
	<textarea
		bind:value
		bind:this={element}
		style:height={inputHeight}
		use:resizeObserver={(e) => {
			textAreaWidth = `${Math.round(e.frame.width)}px`;
		}}
		{...rest}
	></textarea>
{:else}
	<input type="text" bind:value bind:this={element} style:width={inputWidth} {...rest} />
{/if}

<style lang="postcss">
	.label-input {
		min-width: 44px;
		min-height: 20px;
		padding: 2px 4px;
		border: 1px solid transparent;
	}
	.label-input {
		text-overflow: ellipsis;
		width: 100%;
		border-radius: var(--radius-s);
		color: var(--clr-scale-ntrl-0);
		background-color: var(--clr-bg-1);
		outline: none;
		/* not readonly */
		&:not([disabled]):hover {
			background-color: var(--clr-bg-2);
		}
		&:not([disabled]):focus {
			outline: none;
			background-color: var(--clr-bg-2);
			border-color: var(--clr-border-2);
		}
	}
	input {
		height: 20px;
		overflow: hidden;
		white-space: nowrap;
	}
	textarea {
		width: '100%';
		max-height: 76px;
		resize: none;
		word-break: break-word;
		overflow-wrap: break-word;
	}

	.branch-description {
		margin-bottom: 16px;
		margin-right: 16px;
		margin-left: -5px;
	}
	.branch-description::placeholder {
		color: var(--clr-scale-ntrl-60);
	}
</style>
