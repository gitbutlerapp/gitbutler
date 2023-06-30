<script lang="ts">
	import type { LayoutData } from './$types';
	import { Button } from '$lib/components';
	import { events } from '$lib';

	export let data: LayoutData;

	const { projects } = data;
</script>

<div class="h-full w-full bg-light-200 p-8 text-light-900 dark:bg-dark-1000">
	<div class="flex h-full flex-col ">
		{#await projects.load()}
			loading...
		{:then}
			{#if $projects.length == 0}
				<div class="h-fill grid h-full grid-cols-2 items-center gap-4">
					<!-- right box, welcome text -->
					<div class="flex flex-col content-center space-y-4 p-4">
						<div class="m-0 p-0 text-2xl ">
							<div class="font-bold">Welcome to GitButler.</div>
							<div class="mb-1 text-lg ">More than just version control.</div>
						</div>
						<div class="">
							GitButler is a tool to help you manage all the local work you do on your code
							projects.
						</div>
						<div class="">
							Think of us as a <strong>code concierge</strong>, a smart assistant for all the coding
							related tasks you need to do every day.
						</div>
						<ul class="space-y-4 pt-2 pb-4">
							<li class="flex flex-row space-x-3">
								<svg
									xmlns="http://www.w3.org/2000/svg"
									fill="none"
									viewBox="0 0 24 24"
									stroke-width="1.5"
									stroke="currentColor"
									class="h-8 w-8 flex-none"
								>
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										d="M21 16.811c0 .864-.933 1.405-1.683.977l-7.108-4.062a1.125 1.125 0 010-1.953l7.108-4.062A1.125 1.125 0 0121 8.688v8.123zM11.25 16.811c0 .864-.933 1.405-1.683.977l-7.108-4.062a1.125 1.125 0 010-1.953L9.567 7.71a1.125 1.125 0 011.683.977v8.123z"
									/>
								</svg>

								<span class="text-zinc-200"
									>Automatically records everything you do in any of your butlered projects.</span
								>
							</li>
							<li class="flex flex-row space-x-3">
								<svg
									xmlns="http://www.w3.org/2000/svg"
									fill="none"
									viewBox="0 0 24 24"
									stroke-width="1.5"
									stroke="currentColor"
									class="h-8 w-8 flex-none"
								>
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										d="M21 11.25v8.25a1.5 1.5 0 01-1.5 1.5H5.25a1.5 1.5 0 01-1.5-1.5v-8.25M12 4.875A2.625 2.625 0 109.375 7.5H12m0-2.625V7.5m0-2.625A2.625 2.625 0 1114.625 7.5H12m0 0V21m-8.625-9.75h18c.621 0 1.125-.504 1.125-1.125v-1.5c0-.621-.504-1.125-1.125-1.125h-18c-.621 0-1.125.504-1.125 1.125v1.5c0 .621.504 1.125 1.125 1.125z"
									/>
								</svg>

								<span class="text-zinc-200"
									>Simplifys all your Git work, including committing, branching and pushing, to be
									easy and intuitive.
									<span class="text-zinc-500"> (Coming soon)</span>
								</span>
							</li>
							<li class="flex flex-row space-x-3">
								<svg
									xmlns="http://www.w3.org/2000/svg"
									fill="none"
									viewBox="0 0 24 24"
									stroke-width="1.5"
									stroke="currentColor"
									class="h-8 w-8 flex-none"
								>
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										d="M15.75 15.75l-2.489-2.489m0 0a3.375 3.375 0 10-4.773-4.773 3.375 3.375 0 004.774 4.774zM21 12a9 9 0 11-18 0 9 9 0 0118 0z"
									/>
								</svg>

								<span class="text-zinc-200"
									>Helps you not just search for strings or past commits, but find useful context in
									the story of your code.
								</span>
							</li>
						</ul>
						<div class="pt-6">
							<a
								rel="noreferrer"
								target="_blank"
								href="https://docs.gitbutler.com"
								class="mt-4 rounded-lg bg-zinc-700 px-4 py-3 text-base font-semibold leading-7 text-white"
							>
								Learn more <span aria-hidden="true">→</span></a
							>
						</div>
					</div>
					<!-- left box, add a new project -->
					<div class="text-center">
						<svg
							class="mx-auto h-12 w-12 text-gray-400"
							fill="none"
							viewBox="0 0 24 24"
							stroke="currentColor"
							aria-hidden="true"
						>
							<path
								vector-effect="non-scaling-stroke"
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M9 13h6m-3-3v6m-9 1V7a2 2 0 012-2h6l2 2h6a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z"
							/>
						</svg>
						<h3 class="mt-2 text-lg font-semibold text-zinc-300">No projects</h3>
						<p class="mt-1 text-gray-500">Get started by tracking a project you're working on.</p>
						<div class="mt-6">
							<Button color="primary" on:click={() => events.emit('openNewProjectModal')}>
								Start Tracking a Project
							</Button>
						</div>
					</div>
				</div>
			{:else}
				<div class="mb-8 flex flex-row justify-between">
					<div class="text-light-900 dark:text-dark-100">
						<h1 class="mb-2 text-3xl">Your projects</h1>
						<p class="text-lg text-light-700 dark:text-dark-200">
							All the projects that I am currently assisting you with.
						</p>
					</div>
					<div class="self-start">
						<Button color="primary" on:click={() => events.emit('openNewProjectModal')}>
							Add project
						</Button>
					</div>
				</div>
				<div class="flex flex-wrap gap-4">
					{#each $projects as project}
						<a
							class="w-96 overflow-hidden rounded-lg bg-white text-light-900 shadow dark:border dark:border-dark-600 dark:bg-black dark:text-light-200"
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
		{/await}
	</div>
</div>
