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
		show(elt.offsetLeft + elt.clientWidth, elt.offsetTop + elt.clientHeight, item);
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
	<div class="absolute top-0 left-0 z-50 h-full w-full" on:click={onDismiss} on:keydown={onDismiss}>
		<div
			use:recordDimensions
			on:mouseleave={onDismiss}
			on:blur={onDismiss}
			style="position: absolute; top:{pos.y}px; left:{pos.x}px"
			class="flex flex-col rounded-lg border-light-400 bg-white shadow dark:border-dark-500 dark:bg-dark-700 p-1 border"
		>
			<slot {item} />
		</div>
	</div>
{/if}

<style>
</style>
