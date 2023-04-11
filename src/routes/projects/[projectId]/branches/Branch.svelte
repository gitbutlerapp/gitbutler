<script lang="ts">
	import type { Branch } from '$lib/git/branches';
	import Avatar from '$lib/components/Avatar.svelte';
	export let branch: Branch;
	export let remote = false;
	export let head = false;

	function normalizeName(name: string) {
		return name.replace('refs/heads/', '').replace('refs/remotes/', '').replace('origin/', '');
	}
	function relativeDate(ts: number) {
		const formatter = new Intl.RelativeTimeFormat('en-US', {
			numeric: 'always',
			style: 'long'
		});
		const diff = (Date.now() - ts) / 1000;
		if (diff < 60) {
			return formatter.format(-Math.round(diff), 'second');
		} else if (diff < 3600) {
			return formatter.format(-Math.round(diff / 60), 'minute');
		} else if (diff < 86400) {
			return formatter.format(-Math.round(diff / 3600), 'hour');
		} else if (diff < 604800) {
			return formatter.format(-Math.round(diff / 86400), 'day');
		} else if (diff < 2629746) {
			return formatter.format(-Math.round(diff / 604800), 'week');
		} else if (diff < 31556952) {
			return formatter.format(-Math.round(diff / 2629746), 'month');
		} else {
			return formatter.format(-Math.round(diff / 31556952), 'year');
		}
	}

	function classColors(branch: number, direction: string) {
		if (direction === 'ahead') return branch > 0 ? 'bg-blue-900' : 'bg-zinc-700';
		else if (direction === 'behind') {
			if (branch < 1) {
				return 'bg-green-700';
			} else if (branch < 150) {
				return 'bg-blue-800';
			} else {
				return 'bg-red-900';
			}
		}
		return branch > 0 ? 'bg-red-900' : 'bg-gray-900';
	}
</script>

<li
	class="relative {head
		? 'bg-green-900'
		: branch.behind > 200
		? 'bg-zinc-600'
		: 'bg-zinc-700'} rounded-lg px-4 py-4"
>
	<div class="flex items-center justify-between space-x-4">
		<!-- Repo name and link -->
		<div class="flex flex-col items-center">
			<div class="{classColors(branch.ahead, 'ahead')} p-1 text-sm text-center w-10 rounded-t">
				{branch.ahead}
			</div>
			<div
				class="{classColors(
					branch.behind,
					'behind'
				)} p-1 text-sm text-center w-10 text-zinc-400 bg-zinc-900 rounded-b"
			>
				{#if branch.behind > 200}
					200+
				{:else}
					{branch.behind}
				{/if}
			</div>
		</div>
		<div class="flex-1 min-w-0 space-y-1">
			<div class="flex items-center space-x-3 text-black">
				<h2 class="font-medium">
					{#if remote}
						<code class="text-xs text-gray-400 bg-gray-100 rounded-lg"
							>{normalizeName(branch.branch)}</code
						>
					{:else}
						<a href="#"> {normalizeName(branch.name)} </a>
					{/if}
				</h2>
			</div>
			<a href="#" class="group relative flex items-center space-x-2.5">
				<span class="truncate text-sm text-gray-600 group-hover:text-gray-900"
					>{branch.description}
					{branch.upstream}
				</span>
			</a>
			<div class="flex space-x-2 text-xs text-gray-500">
				{#if branch.firstCommitTs > 0}
					{#if !remote}
						<div>
							Created
							{relativeDate(branch.firstCommitTs * 1000)}
						</div>
						<span aria-hidden="true">&middot;</span>
					{/if}
					<div>
						Last work
						{relativeDate(branch.lastCommitTs * 1000)}
					</div>
				{/if}
			</div>
		</div>
		<!-- Repo meta info -->
		<div class="hidden flex-shrink-0 flex-col items-end space-y-1 sm:flex">
			<div class="flex flex-row items-center space-x-4">
				<div class="isolate flex -space-x-2 overflow-hidden">
					{#each branch.authors as email}
						<Avatar {email} />
					{/each}
				</div>
			</div>
		</div>
	</div>
</li>
