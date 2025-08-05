<script lang="ts">
	import { autoSelectBranchNameFeature } from '$lib/config/uiFeatureFlags';
	import { TestId } from '@gitbutler/ui';
	import { clickOutside } from '@gitbutler/ui/utils/clickOutside';
	import { resizeObserver } from '@gitbutler/ui/utils/resizeObserver';

	interface Props {
		name: string;
		disabled?: boolean;
		readonly?: boolean;
		fontSize?: '14' | '15';
		allowClear?: boolean;
		onChange?: (value: string) => void;
		onDblClick?: () => void;
	}

	const {
		name,
		disabled = false,
		fontSize = '14',
		readonly = false,
		allowClear,
		onChange,
		onDblClick
	}: Props = $props();

	let inputEl: HTMLInputElement | undefined = $state();
	let measureWidth = $state(0);

	// Use the actual name or current input value for measurement
	let currentValue = $derived(name);

	const inputWidth = $derived(`${Math.max(measureWidth, 44)}px`);

	function handleInputChange(e: Event) {
		const target = e.currentTarget as HTMLInputElement;
		const value = target.value.trim();

		if (value === name) return;

		if (value === '' && !allowClear) {
			currentValue = name;
			target.value = name;
			return;
		}

		onChange?.(value);
	}

	function handleClick(e: MouseEvent) {
		if (readonly) return;
		e.stopPropagation();
		inputEl?.focus();

		if ($autoSelectBranchNameFeature) {
			inputEl?.select();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' || e.key === 'Escape' || e.key === 'Tab') {
			inputEl?.blur();
		}
	}

	function handleDoubleClick(e: MouseEvent) {
		e.stopPropagation();
		onDblClick?.();
	}

	function handleContextMenu(e: MouseEvent) {
		e.stopPropagation();
	}

	function handleKeypress(e: KeyboardEvent) {
		if (readonly) return;
		e.stopPropagation();
	}

	function handleFocus() {
		currentValue = name;
		if (inputEl) {
			inputEl.value = name;
		}
	}

	function handleInput(e: Event) {
		const target = e.currentTarget as HTMLInputElement;
		currentValue = target.value;
	}
</script>

<!-- Hidden element for measuring text width -->
<span
	data-testid={TestId.BranchNameLabel}
	use:resizeObserver={(e) => {
		measureWidth = Math.round(e.frame.width);
	}}
	class="branch-name-measure-el text-{fontSize} text-bold"
>
	{currentValue}
</span>

<input
	type="text"
	{disabled}
	{readonly}
	bind:this={inputEl}
	bind:value={currentValue}
	onchange={handleInputChange}
	oninput={handleInput}
	title={currentValue}
	class="branch-name-input text-{fontSize} text-bold"
	ondblclick={handleDoubleClick}
	oncontextmenu={handleContextMenu}
	onclick={handleClick}
	onkeypress={handleKeypress}
	onfocus={handleFocus}
	onkeydown={handleKeydown}
	autocomplete="off"
	autocorrect="off"
	spellcheck="false"
	data-remove-from-panning
	use:clickOutside={{
		handler: () => inputEl?.blur()
	}}
	style:width={inputWidth}
/>

<style lang="postcss">
	.branch-name-measure-el,
	.branch-name-input {
		min-width: 44px;
		height: 20px;
		padding: 2px 3px;
		border: 1px solid transparent;
	}

	.branch-name-measure-el {
		display: inline-block;
		visibility: hidden;
		position: fixed;
		width: fit-content;
		border: 2px solid transparent;
		color: black;
		white-space: pre;
		pointer-events: none;
	}

	.branch-name-input {
		width: 100%;
		max-width: 100%;
		overflow: hidden;
		border-radius: var(--radius-s);
		outline: none;
		background-color: transparent;
		color: var(--clr-text-1);
		text-overflow: ellipsis;
		white-space: nowrap;
		transition:
			border var(--transition-fast),
			background-color var(--transition-fast);

		/* not readonly */
		&:not([readonly]):not([disabled]):not(:focus):hover {
			border: 1px solid color-mix(in srgb, var(--clr-scale-ntrl-40), transparent 70%);
		}

		&:not([readonly]):not([disabled]):focus {
			border: 1px solid color-mix(in srgb, var(--clr-scale-ntrl-40), transparent 60%);
			outline: none;
			background-color: var(--clr-bg-1-muted);
		}
	}

	.branch-name-input[readonly] {
		cursor: default;
	}
</style>
