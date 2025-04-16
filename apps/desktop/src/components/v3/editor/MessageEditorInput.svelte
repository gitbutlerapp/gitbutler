<script lang="ts">
	interface Props {
		ref: HTMLInputElement | undefined;
		value: string;
		showCount?: boolean;
		oninput: (e: Event) => void;
		onkeydown: (e: KeyboardEvent) => void;
	}

	let {
		ref = $bindable(),
		value = $bindable(),
		showCount = true,
		oninput,
		onkeydown
	}: Props = $props();

	let charsCount = $state(value.length);
</script>

<!-- svelte-ignore a11y_autofocus -->
<div class="message-editor-input">
	<input
		bind:this={ref}
		placeholder="Commit title"
		class="text-14 text-semibold text-input"
		type="text"
		autofocus
		{value}
		oninput={(e: Event) => {
			const input = e.currentTarget as HTMLInputElement;
			charsCount = input.value.length;
			oninput(e);
		}}
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
		padding: 8px 12px;
		width: 100%;
	}

	.message-editor-input {
		position: relative;
	}

	.message-editor-input__chars-count {
		position: absolute;
		right: 6px;
		bottom: 50%;
		transform: translateY(50%);
		color: var(--clr-text-2);
		padding: 6px;
		background-color: var(--clr-bg-1);

		& span {
			opacity: 0.6;
		}

		&:after {
			content: '';
			position: absolute;
			top: 0;
			left: 0;
			transform: translateX(-90%);
			width: 100%;
			height: 100%;
			background: linear-gradient(
				to right,
				oklch(from var(--clr-bg-1) l c h / 0) 00%,
				var(--clr-bg-1) 90%
			);
		}
	}
</style>
