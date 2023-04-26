<script lang="ts">
	import '../app.postcss';

	import { open } from '@tauri-apps/api/dialog';
	import { toasts, Toaster } from '$lib';
	import tinykeys, { type KeyBindingMap } from 'tinykeys';
	import { format } from 'date-fns';
	import type { LayoutData } from './$types';
	import { BackForwardButtons, Link, CommandPalette, Breadcrumbs } from '$lib/components';
	import { page } from '$app/stores';
	import { derived } from 'svelte/store';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	export let data: LayoutData;
	const { user, posthog, projects, sentry } = data;

	const project = derived([page, projects], ([page, projects]) =>
		projects.find((project) => project.id === page.params.projectId)
	);

	export let commandPalette: CommandPalette;

	onMount(() => {
		const keybindings: KeyBindingMap = {
			// global
			'Meta+k': () => commandPalette.show(),
			'Meta+,': () => goto('/users/'),
			'Meta+Shift+N': async () => {
				const selectedPath = await open({
					directory: true,
					recursive: true
				});
				if (selectedPath === null) return;
				if (Array.isArray(selectedPath) && selectedPath.length !== 1) return;
				const projectPath = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;

				try {
					await projects.add({ path: projectPath });
				} catch (e: any) {
					toasts.error(e.message);
				}
			},

			// project specific
			'Meta+Shift+C': () => $project && goto(`/projects/${$project.id}/commit/`),
			'Meta+T': () => $project && goto(`/projects/${$project.id}/terminal/`),
			'Meta+P': () => $project && goto(`/projects/${$project.id}/`),
			'Meta+Shift+,': () => $project && goto(`/projects/${$project.id}/settings/`),
			'Meta+R': () =>
				$project && goto(`/projects/${$project.id}/player/${format(new Date(), 'yyyy-MM-dd')}`),
			'a i p': () => $project && goto(`/projects/${$project.id}/aiplayground/`)
		};

		return tinykeys(
			window,
			Object.fromEntries(
				Object.entries(keybindings).map(([combo, handler]) => {
					const comboContainsControlKeys =
						combo.includes('Meta') || combo.includes('Alt') || combo.includes('Ctrl');
					return [
						combo,
						(e: KeyboardEvent) => {
							const target = e.target as HTMLElement;
							const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA';
							if (isInput && !comboContainsControlKeys) return;

							commandPalette?.close();

							handler(e);
						}
					];
				})
			)
		);
	});

	user.subscribe(posthog.identify);
	user.subscribe(sentry.identify);
</script>

<div class="flex h-full max-h-full min-h-full flex-col">
	<header
		data-tauri-drag-region
		class="flex select-none flex-row items-center border-b border-zinc-700 pt-1 pb-1 text-zinc-400"
	>
		<div class="ml-24">
			<BackForwardButtons />
		</div>
		<div class="ml-6">
			<Breadcrumbs {project} />
		</div>
		<div class="flex-grow" />
		<div class="mr-6">
			<Link href="/users/">
				{#if $user}
					{#if $user.picture}
						<img class="inline-block h-5 w-5 rounded-full" src={$user.picture} alt="Avatar" />
					{/if}
					<span class="hover:no-underline">{$user.name}</span>
				{:else}
					<span>Connect to GitButler Cloud</span>
				{/if}
			</Link>
		</div>
	</header>

	<div class="flex-auto overflow-auto">
		<slot />
	</div>
	<Toaster />
	<CommandPalette bind:this={commandPalette} {projects} {project} addProject={projects.add} />
</div>
