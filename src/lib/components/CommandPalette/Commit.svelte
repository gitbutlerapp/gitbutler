<script lang="ts">
	import Modal from '../Modal.svelte';
	import { collapsable } from '$lib/paths';
	import * as git from '$lib/git';
	import { onMount } from 'svelte';
	import { success, error } from '$lib/toasts';
	import { createEventDispatcher } from 'svelte';
	import { readable, type Readable } from 'svelte/store';
	import type { Status } from '$lib/git/statuses';
	import { IconRotateClockwise } from '../icons';
	import type { Project } from '$lib/projects';

	const dispatch = createEventDispatcher();

	let statuses = readable<Status[]>([]);

	export let project: Readable<Project>;

	let modal: Modal;
	onMount(() => {
		modal.show();
		git.statuses({ projectId: $project.id ?? '' }).then((r) => (statuses = r));
	});

	let summary = '';
	let description = '';
	let isCommitting = false;
	$: isCommitEnabled = summary.length > 0 && $statuses.some(({ staged }) => staged);

	const reset = () => {
		summary = '';
		description = '';
	};

	const onCommit = (e: SubmitEvent) => {
		const form = e.target as HTMLFormElement;
		const formData = new FormData(form);
		const summary = formData.get('summary') as string;
		const description = formData.get('description') as string;

		isCommitting = true;
		git
			.commit({
				projectId: $project.id,
				message: description.length > 0 ? `${summary}\n\n${description}` : summary,
				push: false
			})
			.then(() => {
				success('Commit created');
				reset();
				dispatch('close');
			})
			.catch(() => {
				error('Failed to commit');
			})
			.finally(() => {
				isCommitting = false;
			});
	};

	const onGroupCheckboxClick = (e: Event) => {
		const target = e.target as HTMLInputElement;
		if (target.checked) {
			git
				.stage({
					projectId: $project.id,
					paths: $statuses.filter(({ staged }) => !staged).map(({ path }) => path)
				})
				.catch(() => {
					error('Failed to stage files');
				});
		} else {
			git
				.unstage({
					projectId: $project.id,
					paths: $statuses.filter(({ staged }) => staged).map(({ path }) => path)
				})
				.catch(() => {
					error('Failed to unstage files');
				});
		}
	};
</script>

<Modal on:close bind:this={modal}>
	<form class="flex flex-col gap-4 rounded p-4" on:submit|preventDefault={onCommit}>
		<header class="w-full border-b border-zinc-700 text-lg font-semibold text-white">
			Commit Your Changes
		</header>

		<fieldset class="flex flex-auto transform flex-col gap-2 overflow-auto transition-all">
			{#if $statuses.length > 0}
				<input
					class="ring-gray-600 block w-full rounded-md border-0 p-4 text-zinc-200 ring-1 ring-inset placeholder:text-gray-400 focus:ring-2 focus:ring-inset focus:ring-blue-600 sm:py-1.5 sm:text-sm sm:leading-6"
					type="text"
					name="summary"
					placeholder="Summary (required)"
					bind:value={summary}
					required
				/>

				<textarea
					rows="4"
					name="description"
					placeholder="Description (optional)"
					bind:value={description}
					class="ring-gray-600 block w-full rounded-md border-0 p-4 text-zinc-200 ring-1 ring-inset placeholder:text-gray-400 focus:ring-2 focus:ring-inset focus:ring-blue-600 sm:py-1.5 sm:text-sm sm:leading-6"
				/>

				{#if isCommitting}
					<div
						class="flex gap-1 rounded bg-[#2563EB] py-2 px-4 disabled:cursor-not-allowed disabled:opacity-50"
					>
						<IconRotateClockwise class="h-5 w-5 animate-spin" />
						<span>Comitting...</span>
					</div>
				{:else}
					<button
						disabled={!isCommitEnabled}
						type="submit"
						class="rounded bg-[#2563EB] py-2 px-4 disabled:cursor-not-allowed disabled:opacity-50"
					>
						Commit changes
					</button>
				{/if}

				<ul
					class="flex flex-auto flex-col overflow-auto rounded border border-card-default bg-card-active"
				>
					<header class="flex w-full items-center py-2 px-4">
						<input
							type="checkbox"
							class="cursor-default disabled:opacity-50"
							on:click={onGroupCheckboxClick}
							checked={$statuses.every(({ staged }) => staged)}
							indeterminate={$statuses.some(({ staged }) => staged) &&
								$statuses.some(({ staged }) => !staged) &&
								$statuses.length > 0}
							disabled={isCommitting}
						/>
						<h1 class="m-auto flex">
							<span class="w-full text-center">{$statuses.length} changed files</span>
						</h1>
					</header>

					{#each $statuses as { path, staged }, i}
						<li
							class:border-b={i < $statuses.length - 1}
							class="flex items-center gap-2 border-gb-700 bg-card-default"
						>
							<input
								type="checkbox"
								class="ml-4 cursor-default py-2 disabled:opacity-50"
								checked={staged}
								on:click|preventDefault={() => {
									staged
										? git.unstage({ projectId: $project.id, paths: [path] }).catch(() => {
												error('Failed to unstage file');
										  })
										: git.stage({ projectId: $project.id, paths: [path] }).catch(() => {
												error('Failed to stage file');
										  });
								}}
							/>
							<span
								class="h-full w-full flex-auto overflow-auto py-2 pr-4 text-left font-mono disabled:opacity-50"
								use:collapsable={{ value: path, separator: '/' }}
							/>
						</li>
					{/each}
				</ul>
			{:else}
				<div class="mx-auto text-center text-white">No changes to commit</div>
			{/if}
		</fieldset>
	</form>
</Modal>
