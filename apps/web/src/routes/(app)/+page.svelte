<script lang="ts">
	import DashboardLayout from '$lib/components/dashboard/DashboardLayout.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { isFound } from '@gitbutler/shared/network/loadable';
	import { getRecentlyPushedProjects } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';

	const routes = getContext(WebRoutesService);
	const recentProjects = getRecentlyPushedProjects();
	let hasRecentProjects = $state(false);

	$effect(() => {
		if (recentProjects.current.length >= 1) {
			const project = recentProjects.current[0];
			hasRecentProjects = true;
			if (isFound(project)) {
				goto(
					routes.projectReviewUrl({
						ownerSlug: project.value.owner,
						projectSlug: project.value.slug
					})
				);
			}
		}
	});
</script>

{#if hasRecentProjects}
	<DashboardLayout>
		<p>You have no recent projects!</p>
	</DashboardLayout>
{:else}
	<div class="empty-state-container">
		<div class="empty-state">
			<div class="empty-state-icon">
				<svg
					width="64"
					height="64"
					viewBox="0 0 24 24"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
				>
					<path
						d="M12 4L3 8L12 12L21 8L12 4Z"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
					<path
						d="M3 16L12 20L21 16"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
					<path
						d="M3 12L12 16L21 12"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				</svg>
			</div>
			<h1>No Recent Projects</h1>
			<p class="description">Get started by creating your first review in GitButler.</p>
			<div class="actions">
				<a
					href="https://docs.gitbutler.com/review/overview"
					target="_blank"
					rel="noopener noreferrer"
				>
					<Button style="pop" icon="open-link">Learn How to Create Reviews</Button>
				</a>
				<a
					href="https://docs.gitbutler.com/features/virtual-branches"
					target="_blank"
					rel="noopener noreferrer"
				>
					<Button kind="outline" icon="open-link">Explore Virtual Branches</Button>
				</a>
			</div>
		</div>
	</div>
{/if}

<style lang="postcss">
	.empty-state-container {
		display: flex;
		justify-content: center;
		align-items: center;
		margin: auto;
	}

	.empty-state {
		max-width: 600px;
		text-align: center;
		padding: 40px;
		background-color: white;
		border-radius: 12px;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.05);
	}

	.empty-state-icon {
		display: flex;
		justify-content: center;
		margin-bottom: 24px;
		color: #2563eb;
	}

	h1 {
		font-size: 24px;
		font-weight: 600;
		margin: 0 0 12px 0;
		color: #1a202c;
	}

	.description {
		font-size: 16px;
		color: #718096;
		margin: 0 0 32px 0;
	}

	.actions {
		display: flex;
		gap: 16px;
		justify-content: center;
		flex-wrap: wrap;
	}

	@media (max-width: 640px) {
		.empty-state {
			padding: 32px 20px;
			margin: 0 20px;
		}

		.actions {
			flex-direction: column;
			gap: 12px;
		}
	}
</style>
