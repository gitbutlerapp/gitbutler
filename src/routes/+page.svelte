<script lang="ts">
	import type { LayoutData } from './$types';
	import { Button, Tooltip } from '$lib/components';
	import { events } from '$lib';

	export let data: LayoutData;

	const { projects } = data;
</script>

<div class="h-full w-full p-8">
	<div class="flex h-full flex-col">
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
				<div class="select-none p-8">
					<div class="flex flex-col">
						<div class="flex flex-row justify-between">
							<div class="pointer-events-none mb-1 select-none text-3xl text-zinc-300">
								My Projects
								<div class="pointer-events-none mb-1 select-none text-lg text-zinc-500">
									All the projects that I am currently assisting you with.
								</div>
							</div>
							<div>
								<Tooltip label="Adds a git repository on your computer to GitButler">
									<Button color="primary" on:click={() => events.emit('openNewProjectModal')}>
										Track a New Project
									</Button>
								</Tooltip>
							</div>
						</div>
						<div class="h-full max-h-screen overflow-auto">
							<div class="mt-4 grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3 ">
								{#each $projects as project}
									<a
										class="project-card-container  text-lg text-dark-300 dark:text-light-300"
										href="/projects/{project.id}/"
									>
										<div
											class="project-card group flex h-full min-h-[125px] flex-col justify-between space-y-1 rounded-lg border border-light-200 bg-light-400 shadow transition duration-150 ease-out hover:bg-light-500 hover:ease-in dark:border-dark-700 dark:bg-dark-300 dark:hover:bg-dark-200"
										>
											<div class="flex-grow-0 px-4 py-4">
												<div class="text-lg text-dark-300 dark:text-light-300">
													{project.title}
												</div>
												<div class="break-words text-base text-dark-100 dark:text-light-100">
													{project.path}
												</div>
											</div>

											<div
												class="flex-grow-0 rounded-b-lg border-t border-zinc-600 bg-zinc-600 px-3 py-1 font-mono text-[12px] text-zinc-300"
											>
												{#if project.api}
													<div class="flex flex-row items-center space-x-2 ">
														<div class="h-2 w-2 rounded-full bg-green-600" />
														<div class="text-zinc-300">Backed-up</div>
													</div>
												{:else}
													<div class="flex flex-row items-center space-x-2 ">
														<div class="h-2 w-2 rounded-full bg-light-100 dark:bg-dark-800" />
														<div class="text-zinc-300">Offline</div>
													</div>
												{/if}
											</div>
										</div>
									</a>
								{/each}
							</div>
						</div>
					</div>
				</div>
			{/if}
		{/await}
	</div>
</div>
