<script lang="ts">
	import type { LayoutData } from './$types';
	import { Button, Tooltip } from '$lib/components';
	import { events } from '$lib';

	export let data: LayoutData;

	const { projects } = data;
</script>

{#await projects.load()}
	Loading...
{:then}
	<div class="flex h-full w-full flex-col bg-light-200 p-8 dark:bg-dark-1000">
		<div class="mb-8 flex flex-row justify-between">
			<div class="text-light-900 dark:text-dark-100">
				<h1 class="mb-2 text-2xl">Your projects</h1>
				<p class="text-lg text-light-700 dark:text-dark-200">
					All the projects Tracked by GitButler
				</p>
			</div>
			{#if $projects.length > 0}
				<div class="self-start">
					<Button color="purple" on:click={() => events.emit('openNewProjectModal')}>
						Add project
					</Button>
				</div>
			{/if}
		</div>
		{#if $projects.length == 0}
			<div
				class="mx-auto flex w-full flex-grow items-center justify-center rounded border border-light-400 bg-light-200 p-8 dark:border-dark-500 dark:bg-dark-1000 "
			>
				<div class="inline-flex w-96 flex-col items-center gap-y-4 text-center">
					<svg
						width="38"
						height="37"
						viewBox="0 0 38 37"
						fill="none"
						class="text-light-600 dark:text-dark-100"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							fill-rule="evenodd"
							clip-rule="evenodd"
							d="M34 4.5C35.6569 4.5 37 5.84315 37 7.5V33.5C37 35.1569 35.6569 36.5 34 36.5H4C2.34315 36.5 1 35.1569 1 33.5V3.5C1 1.84315 2.34315 0.5 4 0.5H13.7574C14.553 0.5 15.3161 0.816071 15.8787 1.37868L18.1213 3.62132C18.6839 4.18393 19.447 4.5 20.2426 4.5H34ZM17 14.5C17 13.3954 17.8954 12.5 19 12.5C20.1046 12.5 21 13.3954 21 14.5V18.5H25C26.1046 18.5 27 19.3954 27 20.5C27 21.6046 26.1046 22.5 25 22.5H21V26.5C21 27.6046 20.1046 28.5 19 28.5C17.8954 28.5 17 27.6046 17 26.5V22.5H13C11.8954 22.5 11 21.6046 11 20.5C11 19.3954 11.8954 18.5 13 18.5H17V14.5Z"
							fill="currentColor"
							stroke="currentColor"
						/>
					</svg>

					<h3 class="text-xl font-medium">Add project</h3>
					<p class="text-light-700 dark:text-dark-200">
						Get started by adding a project you're working on.
					</p>
					<Button color="purple" height="small" on:click={() => events.emit('openNewProjectModal')}>
						Add Git project
					</Button>
				</div>
			</div>
		{:else}
			<div class="flex flex-wrap gap-4">
				{#each $projects as project}
					<a
						class="w-96 overflow-hidden rounded-lg bg-white text-light-900 shadow dark:border dark:border-dark-600 dark:bg-dark-900 dark:text-light-200"
						href="/repo/{project.id}/"
					>
						<div class="p-4">
							<h1 class="text-lg text-light-900 dark:text-dark-100">
								{project.title}
							</h1>
							<p class="text-light-700 dark:text-dark-200">
								{project.path}
							</p>
						</div>

						<div
							class="flex flex-row items-center gap-x-2 bg-light-100 px-4 py-2 text-light-900 dark:bg-dark-600 dark:text-dark-100"
						>
							{#if project.api}
								<div class="h-2 w-2 rounded-full bg-green-600" />
								<div>Backed-up</div>
							{:else}
								<div class="h-2 w-2 rounded-full bg-light-600 dark:bg-dark-200" />
								<div>Offline</div>
							{/if}
						</div>
					</a>
				{/each}
			</div>
		{/if}
	</div>
{/await}
