<script lang="ts">
	import { goto } from '$app/navigation';
	import type { Project } from '$lib/backend/projects';
	import IconButton from '$lib/components/IconButton.svelte';

	export let project: Project;
</script>

<a class="project text-base-12" href="/{project.id}/board">
	<div class="project__info">
		<h1 class="text-base-13">
			{project.title}
		</h1>
		<p class="text-base-12">
			{project.path}
		</p>
	</div>

	<div class="project__actions">
		<IconButton
			icon="settings"
			on:click={(e) => {
				e.stopPropagation();
				e.preventDefault();
				goto(`/${project.id}/settings`);
			}}
		/>
	</div>
</a>

<style lang="postcss">
	.project {
		display: flex;
		gap: var(--space-12);
		align-items: center;
		justify-content: space-between;
		border-radius: var(--radius-m);
		padding: var(--space-4) var(--space-6);
		&:hover {
			background: var(--clr-theme-container-pale);
		}
	}
	.project__info {
		padding: var(--space-4) var(--space-6);
		display: flex;
		flex-direction: column;
		gap: var(--space-8);
		& h1 {
			color: var(--clr-theme-scale-ntrl-30);
		}
		& p {
			color: var(--clr-theme-scale-ntrl-40);
		}
	}
	.project__actions {
	}
</style>
