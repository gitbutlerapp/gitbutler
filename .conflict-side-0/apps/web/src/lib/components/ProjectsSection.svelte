<script lang="ts">
	import { getRelativeTime } from '$lib/utils/dateUtils';

	interface Project {
		name: string;
		slug: string;
		description?: string;
		updated_at?: string;
	}

	interface Props {
		projects: Project[];
		ownerSlug: string;
		sectionTitle?: string;
		loading?: boolean;
	}

	let { projects, ownerSlug, sectionTitle = 'Projects', loading = false }: Props = $props();
</script>

<div class="section-card projects-section">
	<h2 class="section-title">{sectionTitle}</h2>
	{#if loading}
		<div class="loading-state">
			<p>Loading projects...</p>
		</div>
	{:else if projects.length > 0}
		<div class="projects-grid">
			{#each projects as project}
				<div class="project-card">
					<div class="project-header">
						<h3 class="project-name">
							<a href="/{ownerSlug}/{project.slug}">{project.name || project.slug}</a>
						</h3>
						{#if project.updated_at}
							<span class="updated-at">Updated {getRelativeTime(project.updated_at)}</span>
						{/if}
					</div>
					{#if project.description}
						<p class="project-description">{project.description}</p>
					{/if}
				</div>
			{/each}
		</div>
	{:else}
		<div class="empty-state">
			<p>No projects found.</p>
		</div>
	{/if}
</div>

<style>
	.section-card {
		background-color: white;
		border-radius: 8px;
		margin-bottom: 2rem;
		overflow: hidden;
		border: 1px solid color(srgb 0.831373 0.815686 0.807843);
	}

	.section-title {
		font-size: 0.8em;
		margin: 0;
		padding: 12px 15px;
		border-bottom: 1px solid color(srgb 0.831373 0.815686 0.807843);
		background-color: #f3f3f2;
		color: color(srgb 0.52549 0.494118 0.47451);
	}

	/* Projects Section */
	.projects-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
		gap: 1rem;
		padding: 15px;
	}

	.project-card {
		border: 1px solid #e2e8f0;
		border-radius: 6px;
		padding: 1.25rem;
		transition:
			transform 0.2s,
			box-shadow 0.2s;
	}

	.project-card:hover {
		transform: translateY(-2px);
		box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
	}

	.project-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		margin-bottom: 0.5rem;
	}

	.project-name {
		margin: 0;
		font-size: 1.1rem;
		font-weight: 600;
	}

	.project-name a {
		color: #2563eb;
		text-decoration: none;
	}

	.project-name a:hover {
		text-decoration: underline;
	}

	.updated-at {
		font-size: 0.8rem;
		color: #718096;
		white-space: nowrap;
	}

	.project-description {
		font-size: 0.9rem;
		color: #4a5568;
		margin: 0.5rem 0 1rem;
		line-height: 1.5;
	}

	@media (max-width: 768px) {
		.projects-grid {
			grid-template-columns: 1fr;
		}
	}

	.loading-state,
	.empty-state {
		padding: 2rem;
		text-align: center;
		color: #718096;
		font-style: italic;
	}
</style>
