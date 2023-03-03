<script lang="ts">
	import type { Project } from '$lib/projects';
	import type { Session } from '$lib/sessions';
	import { toHumanReadableTime } from '$lib/time';
	import { getContext } from 'svelte';
	import type { Writable } from 'svelte/store';
	import { IconHome, IconChevronRight } from '@tabler/icons-svelte';
	import Popover from '$lib/components/Popover';

	let project: Writable<Project | null | undefined> = getContext('project');
	let session: Writable<Session | null | undefined> = getContext('session');
	let projects: Writable<any> = getContext('projects');
</script>

<div class="flex flex-row items-center space-x-1 bg-zinc-900 text-zinc-400 h-8">
	<a class="hover:text-zinc-200" href="/">
		<IconHome class="w-5 h-5" />
	</a>
	{#if $project}
		<IconChevronRight class="w-5 h-5 text-zinc-700" />
		<div class="flex flex-col">
			<Popover>
				<div slot="button">
					{$project.title}
				</div>
				<div class="flex flex-col">
					<ul class="flex flex-col overflow-y-auto m-2 max-h-[280px]">
						{#each $projects || [] as p}
							<a
								href="/projects/{p.id}"
								class="
								flex items-center
								p-2 rounded hover:bg-zinc-700 cursor-pointer"
							>
								<span class="truncate">
									{p.title}
								</span>

								<span class="grow p-2" />

								{#if $project && $project.id === p.id}
									<svg
										width="15"
										height="11"
										viewBox="0 0 15 11"
										fill="none"
										xmlns="http://www.w3.org/2000/svg"
									>
										<path
											fill-rule="evenodd"
											clip-rule="evenodd"
											d="M14.5816 2.39721C15.1395 1.84882 15.1395 0.959693 14.5816 0.411296C14.0237 -0.137099 13.1192 -0.137099 12.5613 0.411296L5.2381 7.60983L2.43872 4.85811C1.88083 4.30971 0.976311 4.30971 0.418419 4.85811C-0.139473 5.4065 -0.139473 6.29563 0.418419 6.84402L4.22794 10.5887C4.78583 11.1371 5.69036 11.1371 6.24825 10.5887L14.5816 2.39721Z"
											fill="#A1A1AA"
										/>
									</svg>
								{/if}
							</a>
						{/each}
					</ul>
					<span class="w-full border-t border-zinc-700" />
					<!-- svelte-ignore a11y-click-events-have-key-events -->
					<div class="m-2">
						<div class="p-2 rounded hover:bg-zinc-700 cursor-pointer">Add repository...</div>
					</div>
				</div>
			</Popover>
		</div>
	{/if}
	{#if $project && $session}
		<IconChevronRight class="w-5 h-5 text-zinc-700" />
		<a class="hover:text-zinc-200" href="/projects/{$project.id}/sessions/{$session.id}">
			{toHumanReadableTime($session.meta.startTimestampMs)}
			{toHumanReadableTime($session.meta.lastTimestampMs)}
		</a>
	{/if}
</div>
