<script lang="ts">
	import { getCursorPosition, insertImageAtCaret } from '$lib/textEditor/selection';
	import { debounce } from '$lib/utils/debounce';
	import { gifPaginator, GiphyFetch } from '@giphy/js-fetch-api';
	import { Gif } from '@giphy/svelte-components';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { clickOutside } from '@gitbutler/ui/utils/clickOutside';
	import { portal } from '@gitbutler/ui/utils/portal';
	import {} from '@lexical/markdown';
	import { onMount } from 'svelte';
	import { getEditor } from 'svelte-lexical';
	import type { IGif } from '@giphy/js-types';

	const editor = getEditor();

	let position: { left: number; top: number } | undefined = $state();

	onMount(() => {
		const root = editor.getRootElement();
		root?.addEventListener('keydown', monitorKeystrokes);
		return () => {
			root?.removeEventListener('keydown', monitorKeystrokes);
		};
	});

	let query = $state('');
	let loading = $state(false);
	let loadingDiv: HTMLDivElement | undefined = $state();

	/** Key sequence that will show the giphy search box. */
	const matchSequence = '/gif';

	let inputBuffer = '';
	let prefixMatched = false;
	let queryBuffer = '';

	function monitorKeystrokes(e: KeyboardEvent) {
		const key = e.key;
		if (!prefixMatched) {
			if (key.length === 1 && key.match(/[\w /]/)) {
				inputBuffer += key;
				if (inputBuffer.endsWith('/gif ')) {
					prefixMatched = true;
					inputBuffer = ''; // Reset buffer, we now track the term
				}
			} else {
				reset();
			}
		} else {
			if (key.length === 1 && key.match(/\w/)) {
				queryBuffer += key;
			} else if (key === 'Enter') {
				query = queryBuffer;
				position = getCursorPosition();
				e.preventDefault();
				e.stopPropagation();
			} else {
				reset();
			}
		}
	}

	function reset() {
		position = undefined;
		inputBuffer = '';
		prefixMatched = false;
		query = '';
		queryBuffer = '';
		gifs = [];
	}

	/** Bind the fetch function to a query for easy pagination.. */
	function createFetch(query: string) {
		return (offset: number) => giphy.search(query, { offset, limit: 3 });
	}

	// According to Giphy this key does not need to be kept private.
	const giphy = new GiphyFetch('vLyzA6m3XQkcikhfQhAMfqkhAASQdjXy');
	const fetch = $derived(query ? createFetch(query) : undefined);
	// Awaiting this paginator will return a new list that includes
	// existing results in addition to new results.
	const paginator = $derived(fetch ? gifPaginator(fetch) : undefined);
	/** Fetch function that is called when the loading status changes. */
	const fetchMore = debounce(async () => {
		if (paginator) {
			gifs = await paginator();
		}
	}, 500);

	/** List of all loaded search results. */
	let gifs: IGif[] = $state([]);

	/** Fetch more when we see the spinner */
	$effect(() => {
		if (query && loading) fetchMore();
	});

	/* Set loading to true if spinner is visible. */
	$effect(() => {
		if (query && position) {
			const observer = new IntersectionObserver(
				([entry]) => (loading = !!(entry?.isIntersecting && !loading)),
				{ threshold: 0.01 }
			);
			observer.observe(loadingDiv!);
			return () => observer.disconnect();
		}
	});

	function handleClick(gif: IGif) {
		const url = gif['images']['original']['webp'].split('?')[0]!;
		const alt = gif['alt_text'] ?? '';
		insertImageAtCaret(editor, { url, alt, count: matchSequence.length + query.length + 1 });
		reset();
	}
</script>

{#if position}
	<div
		use:portal={'body'}
		class="slash"
		style:left={position.left + 'px'}
		style:top={position.top + 'px'}
		use:clickOutside={{ handler: () => reset() }}
	>
		{#if query}
			<div class="inner">
				{#each gifs as gif}
					<Gif
						{gif}
						width={240}
						hideAttribution={true}
						objectFit="contain"
						on:click={(e) => handleClick(e.detail.gif)}
					/>
				{/each}
				<div class="loading" bind:this={loadingDiv}><Icon name="spinner" /></div>
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.slash {
		position: absolute;
	}
	.inner {
		position: relative;
		padding: 6px;
		max-height: 300px;
		overflow-y: auto;
		border: 1px solid var(--clr-border-1);
		border-radius: var(--radius-s);
		background-color: var(--clr-bg-2);
	}
</style>
