<script lang="ts">
	import type { User } from '$lib/api/cloud/api';
	import type { Project } from '$lib/api/ipc/projects';
	import Link from '$lib/components/Link/Link.svelte';
	import Breadcrumbs from './Breadcrumbs.svelte';

	export let user: User | undefined;
	export let project: Project | undefined;
</script>

<header
	data-tauri-drag-region
	class="bg-color-3 border-color-4 flex h-8 flex-shrink-0 flex-row items-center gap-x-6 border-b"
	style="z-index: 9999;"
>
	<div class="breadcrumb-project-container ml-[80px]">
		<Breadcrumbs {project} />
	</div>
	<div class="flex-grow" />
	<div class="mr-6">
		<Link href="/user/">
			{#if user?.picture}
				<img class="mr-1 inline-block h-5 w-5 rounded-full" src={user.picture} alt="Avatar" />
			{/if}
			{user?.name ?? 'Account'}
		</Link>
	</div>
</header>
