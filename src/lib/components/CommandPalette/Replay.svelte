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
	<div class="mx-2 mb-2 w-full cursor-default select-none">
		<p
			class="commnand-palette-section-header mx-2 cursor-default select-none py-2 text-sm font-semibold text-zinc-300"
		>
			Replay working history
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
					<span class="command-palette-icon icon-replay">
						<svg
							width="20"
							height="20"
							viewBox="0 0 20 20"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path
								fill-rule="evenodd"
								clip-rule="evenodd"
								d="M4.35741 9.89998L8.89997 13.6466L8.89997 6.15333L4.35741 9.89998ZM2.93531 9.12852C2.45036 9.52851 2.45036 10.2715 2.93531 10.6714L8.76369 15.4786C9.41593 16.0166 10.4 15.5526 10.4 14.7071L10.4 5.09281C10.4 4.24735 9.41592 3.7834 8.76368 4.32136L2.93531 9.12852Z"
								fill="#71717A"
							/>
							<path
								fill-rule="evenodd"
								clip-rule="evenodd"
								d="M12.1633 9.89999L15.7 13.3032L15.7 6.49683L12.1633 9.89999ZM10.7488 9.17942C10.34 9.57282 10.34 10.2272 10.7488 10.6206L15.5066 15.1987C16.1419 15.8101 17.2 15.3598 17.2 14.4782L17.2 5.32182C17.2 4.44016 16.1419 3.98992 15.5066 4.60124L10.7488 9.17942Z"
								fill="#71717A"
							/>
						</svg>
					</span>
					<span class="quick-command flex-grow">{listItem.label}</span>
					<span class="quick-command-key">{idx + 1}</span>
				</a>
			{/each}
		</ul>
	</div>
</Modal>
