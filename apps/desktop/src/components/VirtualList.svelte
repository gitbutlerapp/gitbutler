<script lang="ts" module>
	type T = unknown;
</script>

<script lang="ts" generics="T">
	import { SETTINGS } from '$lib/settings/userSettings';
	import { chunk } from '$lib/utils/array';
	import { inject } from '@gitbutler/shared/context';
	import { ScrollableContainer } from '@gitbutler/ui';

	import { tick, type Snippet } from 'svelte';

	type Props = {
		items: Array<T>;
		itemHeight?: number | undefined;
		/** Template for group of items. */
		group: Snippet<[T[]]>;
		/** Number of items grouped together. */
		batchSize: number;
		/** Handler for when scroll has reached with a margin of the bottom. */
		onloadmore?: () => Promise<void>;
	};

	const { items, itemHeight, group, batchSize, onloadmore }: Props = $props();

	let start = $state(0);
	let end = $state(0);

	// Note that `HTMLCollectionOf` is a live list.
	let rows = $state<HTMLCollectionOf<Element>>();

	let heightMap: Array<number> = $state([]);
	let viewport = $state<HTMLDivElement>();
	let viewportHeight = $state(0);
	let resizeObserver: ResizeObserver | null = null;
	let top = $state(0);
	let bottom = $state(0);
	let averageHeight: number = $state(null!);

	const userSettings = inject(SETTINGS);

	const chunks = $derived(chunk(items, batchSize));
	const visible: Array<{ id: number | string; data: T[] }> = $derived(
		chunks.slice(start, end).map((data, i) => {
			return { id: i + start, data };
		})
	);

	async function refresh() {
		if (!viewport || !rows) return;
		const { scrollTop } = viewport;
		await tick(); // wait until the DOM is up to date
		let contentHeight = top - scrollTop;

		let i = start;
		while (contentHeight < viewportHeight && i < chunks.length) {
			let row = rows[i - start];
			if (!row) {
				end = i + 1;
				await tick(); // render the newly visible row
				row = rows[i - start];
			}
			const row_height = (heightMap[i] = itemHeight || (row as HTMLElement)?.offsetHeight) || 0;
			contentHeight += row_height;
			i += 1;
		}
		end = i;
		const remaining = chunks.length - end;
		averageHeight = (top + contentHeight) / end;
		if (end === 0) {
			averageHeight = 0;
		}
		bottom = remaining * averageHeight;
		heightMap.length = chunks.length;
		const totalHeight = heightMap.reduce((x, y) => x + y, 0);
		if (scrollTop + viewportHeight > totalHeight) {
			// If we scroll outside the viewbox scroll to the top.
			viewport.scrollTo(0, totalHeight - viewportHeight);
		}
		if (totalHeight < viewportHeight) {
			onloadmore?.();
		}
		for (const row of rows) {
			resizeObserver?.observe(row);
		}
	}

	async function handleScroll() {
		if (!viewport || !rows) return;
		const { scrollTop } = viewport;
		const oldStart = start;
		for (let v = 0; v < rows.length; v += 1) {
			heightMap[start + v] = itemHeight || (rows[v] as HTMLElement).offsetHeight;
		}
		let i = 0;
		let y = 0;
		while (i < chunks.length) {
			const rowHeight = heightMap[i] || averageHeight;
			if (y + rowHeight > scrollTop) {
				start = i;
				top = y;
				break;
			}
			y += rowHeight;
			i += 1;
		}
		while (i < chunks.length) {
			y += heightMap[i] || averageHeight;
			i += 1;
			if (y > scrollTop + viewportHeight) break;
		}
		end = i;
		const remaining = chunks.length - end;
		averageHeight = y / end;
		while (i < chunks.length) heightMap[i++] = averageHeight;
		bottom = remaining * averageHeight;

		if (start < oldStart) {
			await tick();
			let expectedHeight = 0;
			let actualHeight = 0;
			for (let i = start; i < oldStart; i += 1) {
				if (rows[i - start]) {
					expectedHeight += heightMap[i]!;
					actualHeight += itemHeight || (rows[i - start] as HTMLElement).offsetHeight;
				}
			}
			const d = actualHeight - expectedHeight;
			viewport.scrollTo(0, scrollTop + d);
		}

		const totalHeight = heightMap.reduce((x, y) => x + y, 0);
		if (scrollTop + viewportHeight > totalHeight) {
			viewport.scrollTo(0, totalHeight - viewportHeight);
		}

		if (scrollTop + viewportHeight > totalHeight - 50) {
			onloadmore?.();
		}
	}

	$effect(() => {
		if (viewport) {
			rows = viewport?.getElementsByClassName('list-row');
			resizeObserver = new ResizeObserver(() => {
				refresh();
			});
			return () => {
				resizeObserver?.disconnect();
			};
		}
	});

	$effect(() => {
		if (items && viewportHeight) {
			refresh();
		}
	});
</script>

<ScrollableContainer
	bind:viewport
	whenToShow={$userSettings.scrollbarVisibilityState}
	onscroll={handleScroll}
	bind:viewportHeight
>
	<div class="padded-contents" style:padding-top={top + 'px'} style:padding-bottom={bottom + 'px'}>
		{#each visible as chunk}
			<!-- Note: keying this #each would things much slower. -->
			<div class="list-row">
				{@render group?.(chunk.data)}
			</div>
		{/each}
	</div>
</ScrollableContainer>

<style>
	.list-row {
		display: block;
		overflow: hidden;
		background-color: var(--clr-bg-1);
	}
	.padded-contents {
		display: flex;
		flex-direction: column;
	}
</style>
