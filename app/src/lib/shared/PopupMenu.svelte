<script lang="ts">
	import { clickOutside } from '$lib/clickOutside';

	let pos = { x: 0, y: 0 };
	let menu = { h: 0, w: 0 };
	let browser = { h: 0, w: 0 };
	let showMenu = false;
	let item: any;

	function onDismiss() {
		showMenu = false;
	}

	export function openByMouse(e: MouseEvent, item: any) {
		show(e.clientX, e.clientY, item);
	}

	export function openByElement(elt: HTMLElement, item: any) {
		const rect = elt.getBoundingClientRect();
		show(rect.left, rect.top + rect.height, item);
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
		role="menu"
		tabindex="0"
		use:recordDimensions
		use:clickOutside={{ handler: () => onDismiss() }}
		style="z-index: var(--z-floating); position: absolute; top:{pos.y}px; left:{pos.x}px"
	>
		<slot {item} dismiss={onDismiss} />
	</div>
{/if}
