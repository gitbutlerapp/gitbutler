<script lang="ts">
	import { projectPath } from '$lib/routing';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);

	const project = getProjectByRepositoryId(appState, projectService, projectId);
</script>

<Loading loadable={project.current}>
	{#snippet children(project)}
		<tr class="row">
			<td>
				<div>{project.activeReviewsCount}</div>
			</td>
			<td>
				<div>
					<a href={projectPath({ ownerSlug: project.owner, projectSlug: project.slug })}>
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

	.rounded-top > td {
		padding-top: 8px;

		> div {
			border-top: 1px solid var(--clr-border-2);
		}

		&:first-child > div {
			border-top-left-radius: var(--radius-m);
		}

		&:last-child > div {
			border-top-right-radius: var(--radius-m);
		}
	}

	.rounded-bottom > td {
		&:first-child > div {
			border-bottom-left-radius: var(--radius-m);
		}

		&:last-child > div {
			border-bottom-right-radius: var(--radius-m);
		}
	}
</style>
