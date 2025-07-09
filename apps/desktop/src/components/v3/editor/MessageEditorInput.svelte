<script lang="ts">
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import { onMount, tick } from 'svelte';

	interface Props {
		ref: HTMLTextAreaElement | undefined;
		value: string;
		showCount?: boolean;
		placeholder?: string;
		oninput?: (e: Event) => void;
		onchange?: (value: string) => void;
		onkeydown: (e: KeyboardEvent) => void;
		testId?: string;
	}

	let {
		ref = $bindable(),
		value = $bindable(),
		showCount = true,
		placeholder,
		oninput,
		onchange,
		onkeydown,
		testId
	}: Props = $props();

	let charsCount = $derived(value.length);
	const isCharCount = $derived(showCount && value.length > 0);

	// auto-focus the input when it is mounted
	onMount(async () => {
		if (ref) {
			await tick();
			ref.focus();
		}
	});
</script>

<div class="message-editor-input text-input">
	<Textarea
		{testId}
		bind:textBoxEl={ref}
		bind:value
		{placeholder}
		fontSize={14}
		fontWeight="semibold"
		padding={{ top: 8, right: 24, bottom: 8, left: 12 }}
		oninput={(e: Event) => {
			const input = e.currentTarget as HTMLTextAreaElement;
			charsCount = input.value.length;
			oninput?.(e);
		}}
		unstyled
		onchange={(e) => onchange?.(e.currentTarget.value)}
		{onkeydown}
	/>
	{#if isCharCount}
		<div class="text-12 text-semibold message-editor-input__chars-count">
			<span>{charsCount}</span>
		</div>
	{/if}
</div>

<style lang="postcss">
	.text-input {
		z-index: 0;
		position: relative;
		width: 100%;
		margin-bottom: -1px;
		padding: 8px 12px;
		border-radius: var(--radius-m) var(--radius-m) 0 0;

		&:hover,
		&:focus {
			z-index: 1;
		}

		&.right-padding {
			padding-right: 30px;
		}
	}

	.message-editor-input {
		z-index: 0;
		position: relative;
		width: 100%;
		margin-bottom: -1px;
		padding: 0;
		border-radius: var(--radius-m) var(--radius-m) 0 0;

		&:hover,
		&:focus-within {
			z-index: 1;
		}
	}

	.message-editor-input__chars-count {
		z-index: 1;
		position: absolute;
		top: 6px;
		right: 6px;
		color: var(--clr-text-2);
		pointer-events: none;

		& span {
			opacity: 0.6;
		}
	}
</style>
