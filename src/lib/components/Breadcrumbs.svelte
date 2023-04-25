<script lang="ts">
	import type { Project } from '$lib/api';
	import type { Readable } from 'svelte/store';
	import { IconHome } from './icons';
	import { Tooltip } from '$lib/components';

	export let project: Readable<Project | undefined>;
</script>

<div class="flex flex-row items-center text-zinc-400">
	<a
		Title="GitButler home"
		class="button-home group cursor-default rounded-md p-2 hover:bg-zinc-700 hover:text-zinc-200"
		href="/"
	>
		<IconHome class="hover:fill-zinc-300 hover:text-zinc-200" />
	</a>

	{#if $project}
		<div class="ml-1">
			<Tooltip label="{$project.title} home">
				<a
					class="project-home-button group flex rounded-md py-2 px-2 hover:bg-zinc-700"
					href={`/projects/${$project.id}`}
				>
					<span class="flex h-4 items-center group-hover:text-zinc-200">
						{$project.title}
					</span>
				</a>
			</Tooltip>
		</div>
	{/if}
</div>
