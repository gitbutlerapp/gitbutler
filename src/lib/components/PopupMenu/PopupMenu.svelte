<script lang="ts">
	let pos = { x: 0, y: 0 };
	let menu = { h: 0, w: 0 };
	let browser = { h: 0, w: 0 };
	let showMenu = false;
	let itemId: string | undefined;

	function onDismiss(e: MouseEvent | KeyboardEvent) {
		showMenu = false;
	}

	export function openByMouse(e: MouseEvent, id: string) {
		show(e.clientX, e.clientY, id);
	}

	export function openByElement(elt: HTMLElement, id: string) {
		show(elt.offsetLeft + elt.clientWidth, elt.offsetTop + elt.clientHeight, id);
	}

	function show(x: number, y: number, id: string) {
		itemId = id;
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
		class="absolute top-0 left-0 z-10 h-full w-full"
		on:click={onDismiss}
		on:keydown={onDismiss}
		on:contextmenu|preventDefault={onDismiss}
	>
		<div
			use:recordDimensions
			style="position: absolute; top:{pos.y}px; left:{pos.x}px"
			class="rounded border border-light-400 bg-white shadow dark:border-dark-600 dark:bg-dark-700"
		>
			<slot {itemId} />
		</div>
	</div>
{/if}

<style>
</style>
