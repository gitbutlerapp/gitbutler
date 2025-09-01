<script lang="ts">
	import { featureShowProjectPage } from '$lib/featureFlags';
	import { inject } from '@gitbutler/core/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';

	dayjs.extend(relativeTime);
	type Props = {
		projectId: string;
		roundedTop: boolean;
		roundedBottom: boolean;
	};

	const { projectId, roundedTop, roundedBottom }: Props = $props();

	const routes = inject(WEB_ROUTES_SERVICE);

	const project = getProjectByRepositoryId(projectId);
	const projectRoute = $featureShowProjectPage ? routes.projectPath : routes.projectReviewPath;
</script>

<Loading loadable={project.current}>
	{#snippet children(project)}
		<tr class:rounded-top={roundedTop} class:rounded-bottom={roundedBottom} class="row">
			<td>
				<div>{project.activeReviewsCount}</div>
			</td>
			<td>
				<div>
					<a href={projectRoute({ ownerSlug: project.owner, projectSlug: project.slug })}>
						<p class="slug">{project.owner}/<strong>{project.slug}</strong></p>
					</a>
				</div>
			</td>
			<td>
				<div class="norm">{dayjs(project.createdAt).fromNow()}</div>
			</td>
			<td>
				<div class="norm">{dayjs(project.updatedAt).fromNow()}</div>
			</td>
		</tr>
	{/snippet}
</Loading>

<style lang="postcss">
	.row {
		min-height: 50px;

		> td {
			height: 100%;
			padding: 0;

			> div {
				height: 100%;
				min-height: 50px;
				padding: 16px;

				border-top: none;
				border-bottom: 1px solid var(--clr-border-2);

				background-color: var(--clr-bg-1);
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

	.slug {
		color: var(--clr-text-2);
	}
	.slug strong {
		color: var(--clr-text-1);
	}
</style>
