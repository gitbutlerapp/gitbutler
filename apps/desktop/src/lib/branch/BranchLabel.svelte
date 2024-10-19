<script lang="ts">
	import { resizeObserver } from '@gitbutler/ui/utils/resizeObserver';

	interface Props {
		name: string;
		disabled?: boolean;
		onChange?: (value: string) => void;
		onBlur?: () => void;
	}

	let { name, disabled = false, onChange, onBlur }: Props = $props();

	let inputEl: HTMLInputElement | undefined = $state();
	let labelEl: HTMLSpanElement | undefined = $state();
	let initialName = name;
	let inputVisible = $state(false);
	let inputWidth = $state('');

	export function focusInput() {
		if (disabled) return;
		inputVisible = true;
		setTimeout(() => {
			inputEl?.select();
		});
	}

	function handleBlurInput() {
		if (name === '') name = initialName;
		inputVisible = false;
		onBlur?.();
		setTimeout(() => {
			labelEl?.focus();
		});
	}
</script>

<span
	use:resizeObserver={(e) => {
		inputWidth = `${Math.round(e.frame.width)}px`;
	}}
	class="branch-name-measure-el text-14 text-bold"
>
	{name}
</span>
<div class="branch-name-label-wrap">
	{#if !inputVisible}
		<span
			bind:this={labelEl}
			role="button"
			tabindex={disabled ? undefined : 0}
			class="branch-name-label text-14 text-bold"
			class:disabled
			ondblclick={focusInput}
			onkeydown={(e) => {
				if (e.key === 'Enter') {
					focusInput();
				}
			}}
		>
			{name}
		</span>
	{:else}
		<input
			type="text"
			bind:this={inputEl}
			bind:value={name}
			onchange={(e) => onChange?.(e.currentTarget.value.trim())}
			title={name}
			class="branch-name-input text-14 text-bold"
			onblur={handleBlurInput}
			onfocus={() => {
				initialName = name;
			}}
			onkeydown={(e) => {
				if (e.key === 'Enter' || e.key === 'Escape') {
					handleBlurInput();
				}
			}}
			autocomplete="off"
			autocorrect="off"
			spellcheck="false"
			style:width={inputWidth}
		/>
	{/if}
</div>

<style lang="postcss">
	.branch-name-label-wrap {
		display: flex;
		max-width: 100%;
		overflow: hidden;
	}
	.branch-name-label {
		cursor: default;
		text-overflow: ellipsis;
		white-space: nowrap;
		overflow: hidden;

		&:not(.disabled):hover,
		&:not(.disabled):focus-visible {
			background-color: var(--clr-bg-2);
		}
	}
	.branch-name-measure-el,
	.branch-name-label,
	.branch-name-input {
		min-width: 44px;
		height: 20px;
		padding: 2px 3px;
		border-radius: var(--radius-s);
		color: var(--clr-scale-ntrl-0);
		outline: none;
		border: 1px solid transparent;
	}

	.branch-name-input {
		width: 100%;
	}
	.branch-name-measure-el {
		pointer-events: none;
		visibility: hidden;
		border: 2px solid transparent;
		color: black;
		position: fixed;
		display: inline-block;
		visibility: hidden;
		white-space: pre;
	}
	.branch-name-input {
		max-width: 100%;
		width: 100%;

		/* not readonly */
		/* &:not([disabled]):hover {
			background-color: var(--clr-bg-2);
		} */

		&:not([disabled]):focus {
			outline: none;
			background-color: var(--clr-bg-2);
			border: 1px solid var(--clr-border-2);
		}
	}
</style>
