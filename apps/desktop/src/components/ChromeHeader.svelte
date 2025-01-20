<script lang="ts">
	import SyncButton from '$components/SyncButton.svelte';
	import { platformName } from '$lib/platform/platform';
	import { Project } from '$lib/project/project';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { env } from '$env/dynamic/public';

	const project = getContext(Project);
</script>

<nav>
	<div class="left">
		{#if platformName === 'macos' || env.PUBLIC_TESTING}
			<div class="traffic-lights-placeholder" data-tauri-drag-region></div>
		{/if}
		<SyncButton size="button" />
	</div>
	<div class="center">
		{project.title}
	</div>
	<div class="right">
		<Button kind="ghost" icon="bell" />
	</div>
</nav>

<style>
	nav {
		display: flex;
		justify-content: space-between;
		margin: 16px 16px 0 16px;
	}
</style>
