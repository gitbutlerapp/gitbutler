<script lang="ts">
	import IconChevronRight from '$lib/icons/IconChevronRight.svelte';
	import IconGithub from '$lib/icons/IconGithub.svelte';
	export let projectId: string;
	export let projectTitle: string;
	export let isGitHub: boolean;
	// Optional page title as a prop instead of a slot to ensure header is always draggable
	export let pageTitle: string | undefined = undefined;
</script>

<!-- data-tauri-drag-region needs to be on all elements recursively https://github.com/tauri-apps/tauri/issues/1656#issuecomment-1178574760 -->
<div class="flex h-8 items-center justify-center gap-x-1" data-tauri-drag-region>
	<div class="relative flex items-center gap-x-1" data-tauri-drag-region>
		{#if isGitHub}
			<IconGithub class="h-4 w-4" data-tauri-drag-region></IconGithub>
		{/if}
		<a data-tauri-drag-region class="cursor-default font-semibold" href={`/${projectId}/board`}>
			{projectTitle}</a
		>
		<!-- Center only the project title to avoid content shift during navigatin -->
		<div class="absolute left-full flex items-center" data-tauri-drag-region>
			{#if pageTitle}
				<IconChevronRight class="h-4 w-4" data-tauri-drag-region></IconChevronRight>
				{pageTitle}
			{/if}
		</div>
	</div>
</div>
