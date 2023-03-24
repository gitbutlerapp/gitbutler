<script lang="ts">
	import Modal from '../Modal.svelte';
	import { format, subDays, subWeeks, subMonths, startOfISOWeek, startOfMonth } from 'date-fns';
	import { onDestroy, onMount } from 'svelte';
	import tinykeys from 'tinykeys';
	import { goto } from '$app/navigation';
	import { createEventDispatcher } from 'svelte';
	import { currentProject } from '$lib/current_project';

	const dispatch = createEventDispatcher();

	let selectionIdx = 0;

	let listOptions = [
		{
			label: 'Earlier today',
			href: `/projects/${$currentProject?.id}/player?date=${format(new Date(), 'yyyy-MM-dd')}`
		},
		{
			label: 'Yesterday',
			href: `/projects/${$currentProject?.id}/player?date=${format(
				subDays(new Date(), 1),
				'yyyy-MM-dd'
			)}`
		},
		{
			label: 'The day before yesterday',
			href: `/projects/${$currentProject?.id}/player?date=${format(
				subDays(new Date(), 2),
				'yyyy-MM-dd'
			)}`
		},
		{
			label: 'The beginning of last week',
			href: `/projects/${$currentProject?.id}/player?date=${format(
				startOfISOWeek(subWeeks(new Date(), 1)),
				'yyyy-MM-dd'
			)}`
		},
		{
			label: 'The beginning of last month',
			href: `/projects/${$currentProject?.id}/player?date=${format(
				startOfMonth(subMonths(new Date(), 1)),
				'yyyy-MM-dd'
			)}`
		}
	];

	const gotoDestination = () => {
		goto(listOptions[selectionIdx].href);
		dispatch('close');
	};

	let unsubscribeKeyboardHandler: () => void;

	onMount(() => {
		unsubscribeKeyboardHandler = tinykeys(window, {
			Enter: () => {
				gotoDestination();
			},
			ArrowDown: () => {
				selectionIdx = (selectionIdx + 1) % listOptions.length;
			},
			ArrowUp: () => {
				selectionIdx = (selectionIdx - 1 + listOptions.length) % listOptions.length;
			},
			'Control+n': () => {
				selectionIdx = (selectionIdx + 1) % listOptions.length;
			},
			'Control+p': () => {
				selectionIdx = (selectionIdx - 1 + listOptions.length) % listOptions.length;
			}
		});
	});

	onDestroy(() => {
		unsubscribeKeyboardHandler?.();
	});
</script>

<Modal on:close>
	<div class="mx-2 cursor-default select-none">
		<p class="mx-2 cursor-default select-none py-2 text-sm font-semibold text-zinc-300/80">
			Replay working history from...
		</p>

		<ul class="">
			{#each listOptions as listItem, idx}
				<a
					on:mouseover={() => (selectionIdx = idx)}
					on:focus={() => (selectionIdx = idx)}
					on:click={gotoDestination}
					class="{selectionIdx === idx
						? 'bg-zinc-700/70'
						: ''} flex cursor-default items-center rounded-lg p-2 px-2 outline-none"
					href="/"
				>
					<span class="flex-grow">{listItem.label}</span>
					<span>{idx + 1}</span>
				</a>
			{/each}
		</ul>
	</div>
</Modal>
