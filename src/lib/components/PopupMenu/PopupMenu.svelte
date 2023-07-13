<script lang="ts">
	let pos = { x: 0, y: 0 };
	let menu = { h: 0, w: 0 };
	let browser = { h: 0, w: 0 };
	let showMenu = false;
	let item: any;

	function onDismiss(e: MouseEvent | KeyboardEvent | FocusEvent) {
		showMenu = false;
	}

	export function openByMouse(e: MouseEvent, item: any) {
		show(e.clientX, e.clientY, item);
	}

	export function openByElement(elt: HTMLElement, item: any) {
		const rect = elt.getBoundingClientRect();
		show(rect.left + rect.width, rect.top + rect.height, item);
	}

	function show(x: number, y: number, newItem: any) {
		item = newItem;
		showMenu = true;
		browser = {
			w: window.innerWidth,
			h: window.innerHeight
		};
		pos = {
			x: x,
			y: y
		};

		if (browser.h - pos.y < menu.h) pos.y = pos.y - menu.h;
		if (browser.w - pos.x < menu.w) pos.x = pos.x - menu.w;
	}

	export function recordDimensions(node: HTMLDivElement) {
		let height = node.offsetHeight;
		let width = node.offsetWidth;
		menu = {
			h: height,
			w: width
		};
	}
</script>

{#if showMenu}
	<div
		class="absolute top-0 left-0 z-50 h-full w-full shadow-2xl"
		on:click={onDismiss}
		on:keydown={onDismiss}
	>
		<div
			use:recordDimensions
			on:mouseleave={onDismiss}
			on:blur={onDismiss}
			style="position: absolute; top:{pos.y}px; left:{pos.x}px"
			class="flex flex-col rounded border border-light-400 bg-white p-1 drop-shadow-[0_10px_10px_rgba(0,0,0,0.30)] dark:border-dark-500 dark:bg-dark-700"
		>
			<slot {item} />
		</div>
	</div>
{/if}

<style>
</style>
