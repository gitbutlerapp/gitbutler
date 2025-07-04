<script lang="ts">
	interface Props {
		ref: HTMLInputElement | undefined;
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
</script>

<!-- svelte-ignore a11y_autofocus -->
<div class="message-editor-input">
	<input
		data-testid={testId}
		bind:this={ref}
		{placeholder}
		class="text-14 text-semibold text-input"
		class:right-padding={isCharCount}
		type="text"
		autofocus
		bind:value
		oninput={(e: Event) => {
			const input = e.currentTarget as HTMLInputElement;
			charsCount = input.value.length;
			oninput?.(e);
		}}
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
		position: relative;
	}

	.message-editor-input__chars-count {
		z-index: 1;
		position: absolute;
		right: 6px;
		bottom: 6px;
		padding: 3px;
		background-color: var(--clr-bg-1);
		color: var(--clr-text-2);
		pointer-events: none;

		& span {
			opacity: 0.6;
		}
	}
</style>
