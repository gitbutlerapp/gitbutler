<script lang="ts">
	import { featureShowProjectPage } from '$lib/featureFlags';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);
	const routes = getContext(WebRoutesService);

	const project = getProjectByRepositoryId(appState, projectService, projectId);
	const projectRoute = $featureShowProjectPage ? routes.projectPath : routes.projectReviewPath;
</script>

<Loading loadable={project.current}>
	{#snippet children(project)}
		<tr class="row">
			<td>
				<div>{project.activeReviewsCount}</div>
			</td>
			<td>
				<div>
					<a href={projectRoute({ ownerSlug: project.owner, projectSlug: project.slug })}>
						<p>{project.slug}</p>
					</a>
				</div>
			</td>
			<td>
				<div>{project.createdAt}</div>
			</td>
			<td>
				<div>{project.updatedAt}</div>
			</td>
		</tr>
	{/snippet}
</Loading>

<style lang="postcss">
	.row {
		/*
			This is a magical incantation that lets the divs take up the full
			height of the cell. Nobody knows why this makes any difference
			because it's completly ingnored, but it does!
		*/
		height: 1px;

		> td {
			padding: 0;
			/* This is also part of the magical spell. */
			height: 1px;

			> div {
				height: 100%;

				background-color: var(--clr-bg-1);
				padding: 16px;

				border-top: none;
				border-bottom: 1px solid var(--clr-border-2);
			}

			&:first-child > div {
				border-left: 1px solid var(--clr-border-2);
			}

			&:last-child > div {
				border-right: 1px solid var(--clr-border-2);
			}
		}
	}
</style>
