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

	let charsCount = $state(value.length);
</script>

<!-- svelte-ignore a11y_autofocus -->
<div class="message-editor-input">
	<input
		data-testid={testId}
		bind:this={ref}
		{placeholder}
		class="text-14 text-semibold text-input"
		type="text"
		autofocus
		{value}
		oninput={(e: Event) => {
			const input = e.currentTarget as HTMLInputElement;
			charsCount = input.value.length;
			oninput?.(e);
		}}
		onchange={(e) => onchange?.(e.currentTarget.value)}
		{onkeydown}
	/>
	{#if charsCount > 0 && showCount}
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
	}

	.message-editor-input {
		position: relative;
	}

	.message-editor-input__chars-count {
		z-index: 1;
		position: absolute;
		right: 6px;
		bottom: 50%;
		padding: 6px;
		transform: translateY(50%);
		background-color: var(--clr-bg-1);
		color: var(--clr-text-2);

		& span {
			opacity: 0.6;
		}

		&:after {
			position: absolute;
			top: 0;
			left: 0;
			width: 100%;
			height: 100%;
			transform: translateX(-90%);
			background: linear-gradient(
				to right,
				oklch(from var(--clr-bg-1) l c h / 0) 00%,
				var(--clr-bg-1) 90%
			);
			content: '';
		}
	}
</style>
