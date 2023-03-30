<script lang="ts">
	import Modal from '../Modal.svelte';
	import { format, subDays, subWeeks, subMonths, startOfISOWeek, startOfMonth } from 'date-fns';
	import { onMount } from 'svelte';
	import tinykeys from 'tinykeys';
	import { goto } from '$app/navigation';
	import { createEventDispatcher } from 'svelte';
	import type { Project } from '$lib/projects';
	import type { Readable } from 'svelte/store';

	export let project: Readable<Project>;

	const dispatch = createEventDispatcher<{ close: void }>();

	let selectionIdx = 0;

	let listOptions = [
		{
			label: 'Earlier today',
			href: `/projects/${$project.id}/player/${format(new Date(), 'yyyy-MM-dd')}/`
		},
		{
			label: 'Yesterday',
			href: `/projects/${$project.id}/player/${format(subDays(new Date(), 1), 'yyyy-MM-dd')}/`
		},
		{
			label: 'The day before yesterday',
			href: `/projects/${$project.id}/player/${format(subDays(new Date(), 2), 'yyyy-MM-dd')}/`
		},
		{
			label: 'The beginning of last week',
			href: `/projects/${$project.id}/player/${format(
				startOfISOWeek(subWeeks(new Date(), 1)),
				'yyyy-MM-dd'
			)}/`
		},
		{
			label: 'The beginning of last month',
			href: `/projects/${$project.id}/player/${format(
				startOfMonth(subMonths(new Date(), 1)),
				'yyyy-MM-dd'
			)}/`
		}
	];

	const gotoDestination = () => {
		goto(listOptions[selectionIdx].href);
		dispatch('close');
	};

	let modal: Modal;
	onMount(() => {
		modal.show();
		const unsubscribeKeyboardHandler = tinykeys(window, {
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
		return () => {
			unsubscribeKeyboardHandler();
		};
	});
</script>

<Modal on:close bind:this={modal}>
	<div class="mx-2 cursor-default select-none">
		<p class="mx-2 cursor-default select-none py-2 text-sm font-semibold text-zinc-300">
			Replay working history from...
		</p>

		<ul class="quick-command-list">
			{#each listOptions as listItem, idx}
				<a
					on:mouseover={() => (selectionIdx = idx)}
					on:focus={() => (selectionIdx = idx)}
					on:click={gotoDestination}
					class="{selectionIdx === idx
						? 'bg-zinc-50/10'
						: ''} quick-command-item flex cursor-default items-center"
					href="/"
				>
					<span class="quick-command flex-grow">{listItem.label}</span>
					<span class="quick-command-key">{idx + 1}</span>
				</a>
			{/each}
		</ul>
	</div>
</Modal>
