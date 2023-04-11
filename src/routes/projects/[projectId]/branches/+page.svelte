<script lang="ts">
	import Branch from './Branch.svelte';

	import { derived } from 'svelte/store';
	import type { PageData } from './$types';

	export let data: PageData;
	$: branches = derived(data.branches, (branches) => branches);
	$: head = derived(data.head, (head) => head);

	// pull head out of branches
	$: head_branch = $branches
		.filter((branch) => branch.name.replace('refs/heads/', '') === $head)
		.pop();

	// get local branches
	$: heads = $branches
		.filter(
			(branch) => branch.name !== $head && branch.branch.includes('refs/heads/') && branch.ahead > 0
		)
		.sort((a, b) => (a.lastCommitTs > b.lastCommitTs ? -1 : 1));

	// pull remotes out of branches
	$: remotes = $branches
		.filter((branch) => branch.branch.includes('refs/remotes/') && branch.ahead > 0)
		.sort((a, b) => (a.lastCommitTs > b.lastCommitTs ? -1 : 1));
</script>

<div class="mx-auto w-full max-w-7xl flex-grow lg:flex xl:px-8">
	<!-- Left sidebar & main wrapper -->
	<div class="min-w-0 flex-1 xl:flex">
		<!-- Projects List -->
		<div class="lg:min-w-0 lg:flex-1">
			<div class="p-6">
				<div class="flex flex-row justify-between items-center">
					<div class="flex flex-col">
						<h1 class="text-lg font-medium">Branches</h1>
						<div class="text-sm text-gray-400">Safe contexts that you can do work in</div>
					</div>
					<div class="flex flex-row">
						<button
							type="button"
							class="inline-flex justify-center rounded-md border border-gray-300 px-4 py-2 text-sm font-medium text-gray-700 shadow-sm hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2"
							id="sort-menu-button"
							aria-expanded="false"
							aria-haspopup="true"
						>
							Filter
							<!-- Heroicon name: mini/chevron-down -->
							<svg
								class="ml-2.5 -mr-1.5 h-5 w-5 text-gray-400"
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 20 20"
								fill="currentColor"
								aria-hidden="true"
							>
								<path
									fill-rule="evenodd"
									d="M5.23 7.21a.75.75 0 011.06.02L10 11.168l3.71-3.938a.75.75 0 111.08 1.04l-4.25 4.5a.75.75 0 01-1.08 0l-4.25-4.5a.75.75 0 01.02-1.06z"
									clip-rule="evenodd"
								/>
							</svg>
						</button>
						<button
							type="button"
							class="ml-4 inline-flex items-center rounded-full border border-transparent bg-blue-600 p-3 text-white shadow-sm hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2"
						>
							<!-- Heroicon name: outline/plus -->
							<svg
								class="h-3 w-3"
								xmlns="http://www.w3.org/2000/svg"
								fill="none"
								viewBox="0 0 24 24"
								stroke-width="2.0"
								stroke="currentColor"
								aria-hidden="true"
							>
								<path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
							</svg>
						</button>
					</div>
				</div>
			</div>

			{#if $branches.length === 0}
				<div class="pl-4 pr-6 pt-4 pb-4 sm:pl-6 lg:pl-8 xl:pl-6 xl:pt-6">
					<div class="flex flex-row justify-between items-center">
						<div class="flex flex-col">
							<h1 class="text-lg font-medium">No branches</h1>
							<div class="text-sm text-gray-400">Create a branch to start working</div>
						</div>
					</div>
				</div>
			{:else if $head}
				<!-- list of branches -->
				<ul role="list" class="px-4 space-y-2">
					{#if head_branch}
						<Branch branch={head_branch} head={true} />
					{/if}
					{#each heads as branch}
						<Branch {branch} />
					{/each}
				</ul>
			{/if}
		</div>
	</div>

	<!-- Activity feed -->
	<div class="bg-gray-50 pr-4 sm:pr-6 lg:flex-shrink-0 lg:pr-8 xl:pr-0">
		<div class="pl-6 lg:w-96">
			<div class="pt-6 pb-2">
				<h2 class="text-sm font-semibold">Team Branches</h2>
				<div class="text-sm text-gray-400">Active branches that your teammates are working on</div>
			</div>
			<div>
				<ul role="list" class="space-y-2">
					{#each remotes as branch}
						<Branch {branch} remote={true} />
					{/each}
				</ul>
				<div class="border-t border-gray-200 py-4 text-sm">
					<a href="#" class="font-semibold text-blue-600 hover:text-indigo-900">
						View all team branches
						<span aria-hidden="true"> &rarr;</span>
					</a>
				</div>
			</div>
		</div>
	</div>
</div>
