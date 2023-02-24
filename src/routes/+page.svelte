<script lang="ts">
	import { open } from '@tauri-apps/api/dialog';
	import type { LayoutData } from './$types';
	export let data: LayoutData;

	const { projects } = data;

	const onAddLocalRepositoryClick = async () => {
		const selectedPath = await open({
			directory: true,
			recursive: true
		});
		if (selectedPath === null) return;
		if (Array.isArray(selectedPath) && selectedPath.length !== 1) return;
		const projectPath = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;

		const projectExists = $projects.some((p) => p.path === projectPath);
		if (projectExists) return;

		await projects.add({ path: projectPath });
	};
</script>

<div class="w-full h-full p-8">
	<div class="flex flex-col h-full">
		{#if $projects.length == 0}
			<div class="h-fill grid grid-cols-2 gap-4 items-center h-full">
				<!-- right box, welcome text -->
				<div class="flex flex-col space-y-4 content-center p-4">
					<div class="text-xl text-zinc-300 p-0 m-0">
						<div class="font-bold">Welcome to GitButler.</div>
						<div class="text-lg text-zinc-300 mb-1">More than just version control.</div>
					</div>
					<div class="">
						GitButler is a tool to help you manage all the local work you do on your code projects.
					</div>
					<div class="">
						Think of us as a <strong>code concierge</strong>, a smart assistant for all the coding
						related tasks you need to do every day.
					</div>
					<ul class="text-zinc-400 pt-2 pb-4 space-y-4">
						<li class="flex flex-row space-x-3">
							<svg
								xmlns="http://www.w3.org/2000/svg"
								fill="none"
								viewBox="0 0 24 24"
								stroke-width="1.5"
								stroke="currentColor"
								class="w-8 h-8 flex-none"
							>
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									d="M21 16.811c0 .864-.933 1.405-1.683.977l-7.108-4.062a1.125 1.125 0 010-1.953l7.108-4.062A1.125 1.125 0 0121 8.688v8.123zM11.25 16.811c0 .864-.933 1.405-1.683.977l-7.108-4.062a1.125 1.125 0 010-1.953L9.567 7.71a1.125 1.125 0 011.683.977v8.123z"
								/>
							</svg>

							<span class="text-zinc-200"
								>Automatically recording everything you do in any of your butlered projects.</span
							>
						</li>
						<li class="flex flex-row space-x-3">
							<svg
								xmlns="http://www.w3.org/2000/svg"
								fill="none"
								viewBox="0 0 24 24"
								stroke-width="1.5"
								stroke="currentColor"
								class="w-8 h-8 flex-none"
							>
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									d="M21 11.25v8.25a1.5 1.5 0 01-1.5 1.5H5.25a1.5 1.5 0 01-1.5-1.5v-8.25M12 4.875A2.625 2.625 0 109.375 7.5H12m0-2.625V7.5m0-2.625A2.625 2.625 0 1114.625 7.5H12m0 0V21m-8.625-9.75h18c.621 0 1.125-.504 1.125-1.125v-1.5c0-.621-.504-1.125-1.125-1.125h-18c-.621 0-1.125.504-1.125 1.125v1.5c0 .621.504 1.125 1.125 1.125z"
								/>
							</svg>

							<span class="text-zinc-200"
								>Simplifying all your Git work, including committing, branching and pushing, to be
								easy and intuitive.
							</span>
						</li>
						<li class="flex flex-row space-x-3">
							<svg
								xmlns="http://www.w3.org/2000/svg"
								fill="none"
								viewBox="0 0 24 24"
								stroke-width="1.5"
								stroke="currentColor"
								class="w-8 h-8 flex-none"
							>
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									d="M15.75 15.75l-2.489-2.489m0 0a3.375 3.375 0 10-4.773-4.773 3.375 3.375 0 004.774 4.774zM21 12a9 9 0 11-18 0 9 9 0 0118 0z"
								/>
							</svg>

							<span class="text-zinc-200"
								>Helping you not just search for strings or past commits, but find useful context in
								the story of your code.
							</span>
						</li>
					</ul>
					<div class="pt-6">
						<a
							rel="noreferrer"
							target="_blank"
							href="https://help.gitbutler.com"
							class="text-base font-semibold leading-7 text-white bg-zinc-700 px-4 py-3 rounded-lg mt-4"
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
						<button
							on:click={onAddLocalRepositoryClick}
							type="button"
							class="inline-flex items-center rounded-md border border-transparent bg-blue-600 px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
						>
							Start Tracking a Project
						</button>
					</div>
				</div>
			</div>
		{:else}
			<div class="select-none p-8">
				<div class="flex flex-col">
					<div class="flex flex-row justify-between">
						<div class="text-xl text-zinc-300 mb-1">
							My Projects
							<div class="text-lg text-zinc-500 mb-1">
								All the projects that I am currently helping you with.
							</div>
						</div>
						<div>
							<button
								on:click={onAddLocalRepositoryClick}
								type="button"
								class="inline-flex items-center rounded-md border border-transparent bg-blue-600 px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
							>
								Track a New Project
							</button>
						</div>
					</div>
					<div class="h-full max-h-screen overflow-auto">
						<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mt-4">
							{#each $projects as project}
								<div class="flex flex-col justify-between space-y-1 bg-zinc-700 rounded-lg shadow">
									<div class="px-4 py-4 flex-grow-0">
										<a
											class="hover:text-zinc-200 text-zinc-300 text-lg"
											href="/projects/{project.id}/">{project.title}</a
										>
										<div class="text-zinc-500 font-mono">
											{project.path}
										</div>
									</div>
									<div
										class="flex-grow-0 text-zinc-500 font-mono border-t border-zinc-600 bg-zinc-600 rounded-b-lg px-3 py-1"
									>
										{#if project.api}
											<div class="flex flex-row items-center space-x-2 ">
												<div class="w-2 h-2 bg-green-700 rounded-full" />
												<div class="text-zinc-400">syncing</div>
											</div>
										{:else}
											<div class="flex flex-row items-center space-x-2 ">
												<div class="w-2 h-2 bg-gray-400 rounded-full" />
												<div class="text-zinc-400">offline</div>
											</div>
										{/if}
									</div>
								</div>
							{/each}
						</div>
					</div>
				</div>
			</div>

			<div class="absolute bottom-0 left-0 w-full">
				<div
					class="flex items-center flex-shrink-0 p-4 h-18 border-t select-none border-zinc-700 bg-zinc-900"
				>
					<div class="text-sm text-zinc-300">Timeline</div>
				</div>
			</div>
		{/if}
	</div>
</div>
