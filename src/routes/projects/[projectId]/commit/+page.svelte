<script lang="ts">
	import type { PageData } from './$types';
	import { collapsable } from '$lib/paths';
	import { derived, writable } from 'svelte/store';
	import DiffViewer from '$lib/components/DiffViewer.svelte';

	export let data: PageData;
	const { statuses, diffs } = data;

	let selectedFiles = $statuses.map(({ path }) => path);
	const selectedDiffPath = writable($statuses.at(0)?.path);
	const selectedDiff = derived(
		[diffs, selectedDiffPath],
		([diffs, selectedDiffPath]) => diffs[selectedDiffPath]
	);

	const onCommit = (e: SubmitEvent) => {
		const form = e.target as HTMLFormElement;
		const formData = new FormData(form);
		const summary = formData.get('summary') as string;
		const description = formData.get('description') as string;
		const paths = formData.getAll('path') as string[];

		// TODO
	};

	const onGroupCheckboxClick = (e: Event) => {
		const target = e.target as HTMLInputElement;
		if (target.checked) {
			selectedFiles = $statuses.map(({ path }) => path);
		} else {
			selectedFiles = [];
		}
	};

	let summary = '';

	$: isEnabled = summary.length > 0 && selectedFiles.length > 0;
</script>

<div id="commit-page" class="flex h-full w-full gap-2 p-2">
	<div>
		<h1 class="px-2 py-1 text-xl font-bold">Commit</h1>

		<form on:submit|preventDefault={onCommit} class="flex w-1/3 min-w-[500px] flex-col gap-4">
			<ul class="flex w-full flex-col rounded border border-gb-900 bg-gb-800">
				<header class="flex w-full items-center py-2 px-4">
					<input
						type="checkbox"
						class="cursor-pointer"
						on:click={onGroupCheckboxClick}
						checked={$statuses.length === selectedFiles.length}
						indeterminate={$statuses.length > selectedFiles.length && selectedFiles.length > 0}
					/>
					<h1 class="m-auto flex">
						<span class="w-full text-center">{$statuses.length} changed files</span>
					</h1>
				</header>

				{#each $statuses as { path }, i}
					<li
						class:bg-gb-700={$selectedDiffPath === path}
						class:hover:bg-gb-750={$selectedDiffPath !== path}
						class:border-b={i < $statuses.length - 1}
						class="flex items-center gap-2 border-gb-700 bg-gb-900"
					>
						<input
							class="ml-4 cursor-pointer py-2"
							name="path"
							type="checkbox"
							bind:group={selectedFiles}
							value={path}
						/>
						<label class="flex w-full" for="path">
							<button
								on:click|preventDefault={() => ($selectedDiffPath = path)}
								type="button"
								class="h-full w-full py-2 pr-4 text-left font-mono "
								use:collapsable={{ value: path, separator: '/' }}
							/>
						</label>
					</li>
				{/each}
			</ul>

			<input
				name="summary"
				class="rounded border border-gb-900 bg-gb-800 p-3"
				type="text"
				placeholder="Summary (required)"
				bind:value={summary}
				required
			/>

			<textarea
				name="description"
				class="rounded border border-gb-900 bg-gb-800 p-3"
				rows="10"
				placeholder="Description (optional)"
			/>

			<div class="flex justify-between">
				<input
					disabled={!isEnabled}
					type="submit"
					value="Commit changes"
					class="rounded bg-[#2563EB] py-2 px-4 text-lg disabled:cursor-not-allowed disabled:opacity-50"
				/>

				<button
					type="button"
					class="rounded bg-gradient-to-b from-[#623871] to-[#502E5C] py-2 px-4 text-lg"
				>
					✨ Generate commit message
				</button>
			</div>
		</form>
	</div>

	<div id="preview" class="m-2 flex flex-auto overflow-auto">
		{#if $selectedDiff !== undefined}
			<DiffViewer diff={$selectedDiff} path={$selectedDiffPath} />
		{:else}
			<div class="flex h-full w-full flex-col items-center justify-center">
				<p class="text-lg">Unable to load diff</p>
			</div>
		{/if}
	</div>
</div>
