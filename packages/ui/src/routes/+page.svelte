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
	<SelectProject projects={$projects$} user={$user$} />
{/if}

<style lang="postcss">
</style>
