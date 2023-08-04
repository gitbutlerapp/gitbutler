<script lang="ts">
	import { IconChevronDown, IconChevronUp } from '$lib/icons';
	export let justify: 'start' | 'end' = 'start';
	export let disabled = false;
	let expanded = false;

	function handleWindowKeyDown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			expanded = false;
		}
	}

	function clickOutside(element: HTMLElement, callback: () => void) {
		const handleClick = (event: Event) => {
			const target = event.target as HTMLElement;
			if (!target) {
				return;
			}
			if (!element.contains(target)) {
				callback();
			}
		};
		document.body.addEventListener('click', handleClick);
		return {
			update(newCallback: () => void) {
				callback = newCallback;
			},
			destroy() {
				document.removeEventListener('click', handleClick, true);
			}
		};
	}
</script>

<svelte:window on:keydown={handleWindowKeyDown} />

<div use:clickOutside={() => (expanded = false)} class="relative inline-block text-left">
	<button
		{disabled}
		on:click={() => (expanded = !expanded)}
		class:hover:bg-light-150={!disabled}
		class:hover:dark:bg-dark-700={!disabled}
		class="flex w-56 items-center justify-center gap-1 rounded-none border-r border-light-500 px-1 py-2 leading-none dark:border-dark-500"
	>
		<div>
			<slot name="label" />
		</div>
		<div class:invisible={disabled}>
			{#if expanded}
				<IconChevronUp class="h-3 w-3" />
			{:else}
				<IconChevronDown class="h-3 w-3" />
			{/if}
		</div>
	</button>
	{#if expanded}
		<div
			class="absolute left-0 z-50 -ml-px min-h-full
			w-56
			origin-top-right border-b border-l border-r border-light-500
			bg-light-100 shadow-lg focus:outline-none
			dark:border-dark-500
			dark:bg-dark-800"
			class:left-0={justify === 'start'}
			class:right-0={justify === 'end'}
			role="menu"
			aria-orientation="vertical"
			aria-labelledby="menu-button"
			tabindex="-1"
		>
			<div class="py-1" role="none">
				<slot name="content" />
			</div>
		</div>
	{/if}
</div>
