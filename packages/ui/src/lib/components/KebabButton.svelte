<script lang="ts">
	import Button from '$components/Button.svelte';
	import Icon from '$components/Icon.svelte';

	interface Props {
		flat?: boolean;
		activated?: boolean;
		contextElement?: HTMLElement;
		contextElementSelected?: boolean;
		testId?: string;
		oncontext?: (event: MouseEvent) => boolean | void;
		onclick?: (element: HTMLElement) => void;
		el?: HTMLElement;
	}

	let {
		flat = false,
		activated = false,
		contextElement,
		testId,
		onclick,
		oncontext,
		el = $bindable()
	}: Props = $props();

	let visible = $state(false);
	let buttonElement = $state<HTMLElement>();
	let isContextMenuOpen = $state(false);
	let openedViaClick = $state(false);

	// Keep el in sync with buttonElement
	$effect(() => {
		el = buttonElement;
	});

	function onMouseEnter() {
		if (!flat) return;
		visible = true;
	}

	function onMouseLeave() {
		if (!flat) return;
		visible = false;
	}

	function onFocus() {
		if (!flat) return;
		visible = true;
	}

	function onBlur() {
		if (!flat) return;
		visible = false;
	}

	function onContextMenu(e: MouseEvent) {
		e.preventDefault(); // Prevent default to avoid browser context menu
		isContextMenuOpen = true;
		openedViaClick = false; // Context menu opened via right-click
		oncontext?.(e);
	}

	function onClick(e: MouseEvent) {
		e.stopPropagation();
		e.preventDefault();
		isContextMenuOpen = !isContextMenuOpen;
		openedViaClick = isContextMenuOpen; // Track if opened via click
		onclick?.(e.currentTarget as HTMLElement);
	}

	function onKeyDown(e: KeyboardEvent) {
		if (e.key !== 'Enter') {
			return;
		}
		e.stopPropagation();
		e.preventDefault();
		isContextMenuOpen = !isContextMenuOpen;
		openedViaClick = isContextMenuOpen; // Track if opened via click
		onclick?.(e.currentTarget as HTMLElement);
	}

	$effect(() => {
		if (contextElement) {
			contextElement.addEventListener('contextmenu', onContextMenu);
			contextElement.addEventListener('mouseenter', onMouseEnter);
			contextElement.addEventListener('mouseleave', onMouseLeave);
			contextElement.addEventListener('focus', onFocus);
			contextElement.addEventListener('blur', onBlur);
			return () => {
				contextElement.removeEventListener('contextmenu', onContextMenu);
				contextElement.removeEventListener('mouseenter', onMouseEnter);
				contextElement.removeEventListener('mouseleave', onMouseLeave);
				contextElement.removeEventListener('focus', onFocus);
				contextElement.removeEventListener('blur', onBlur);
			};
		}
	});

	// Close context menu when clicking outside
	$effect(() => {
		if (isContextMenuOpen) {
			function handleClickOutside(e: MouseEvent) {
				if (buttonElement && !buttonElement.contains(e.target as Node)) {
					isContextMenuOpen = false;
					openedViaClick = false; // Reset when closing
				}
			}
			document.addEventListener('click', handleClickOutside);
			return () => {
				document.removeEventListener('click', handleClickOutside);
			};
		}
	});
</script>

{#if flat}
	<button
		bind:this={buttonElement}
		type="button"
		class="kebab-btn"
		class:visible={visible || isContextMenuOpen}
		class:activated={activated || (isContextMenuOpen && openedViaClick)}
		onclick={onClick}
		onkeypress={onKeyDown}
		data-testid={testId}
	>
		<Icon name="kebab" />
	</button>
{:else}
	<Button
		bind:el={buttonElement}
		{testId}
		size="tag"
		icon="kebab"
		kind="ghost"
		activated={activated || (isContextMenuOpen && openedViaClick)}
		onclick={onClick}
		onkeydown={onKeyDown}
	/>
{/if}

<style lang="postcss">
	.kebab-btn {
		display: flex;
		display: none;
		padding: 0 3px;
		color: var(--clr-text-1);

		&.visible {
			display: flex;
			opacity: 0.5;
		}

		&.activated,
		&:hover,
		&:focus-within {
			display: flex;
			opacity: 1;
		}
	}
</style>
