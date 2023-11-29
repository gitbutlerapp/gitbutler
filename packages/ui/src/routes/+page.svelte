<script lang="ts">
	import type { LayoutData } from './$types';
	import SelectProject from './components/SelectProject.svelte';

	export let data: LayoutData;

	const { projectService, userService } = data;
	$: projects$ = projectService.projects$;
	$: user$ = userService.user$;
</script>

{#if !$projects$}
	Loading...
{:else}
	<div class="absolute h-4 w-full" data-tauri-drag-region></div>
	<div class="homepage">
		<div class="homepage__content">
			<SelectProject projects={$projects$} user={$user$} />
		</div>
	</div>
{/if}

<style lang="postcss">
	.homepage {
		display: flex;
		align-items: center;
		flex-direction: column;
		justify-content: center;
		flex-grow: 1;
	}
	.homepage__content {
		display: flex;
		flex-direction: column;
		max-width: 420px;
		gap: var(--space-8);
	}
</style>
