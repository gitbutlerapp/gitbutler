<script lang="ts">
	import { goto } from '$app/navigation';
	import { inject } from '@gitbutler/core/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound } from '@gitbutler/shared/network/loadable';
	import {
		getProjectByRepositoryId,
		getRecentlyPushedProjects
	} from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Icon } from '@gitbutler/ui';

	type Props = {
		showOwner?: boolean;
		repositoryId: string;
		inRecentSection?: boolean;
	};

	const { showOwner = false, repositoryId, inRecentSection = true }: Props = $props();

	const routes = inject(WEB_ROUTES_SERVICE);

	const project = $derived(getProjectByRepositoryId(repositoryId));
	const projectPageParams = $derived(routes.isProjectPageSubset);

	const recentProjects = getRecentlyPushedProjects();
	const focused = $derived.by(() => {
		if (!projectPageParams) return false;
		if (!isFound(project.current)) return;
		const projectIsRecentlyPushed = recentProjects.current.some(
			(recentProject) => recentProject.id === repositoryId
		);
		const sectionIsSelected =
			projectPageParams.ownerSlug === project.current.value.owner &&
			projectPageParams.projectSlug === project.current.value.slug;

		if (projectIsRecentlyPushed && inRecentSection && sectionIsSelected) return true;
		if (projectIsRecentlyPushed && !inRecentSection && sectionIsSelected) return false;

		return sectionIsSelected;
	});
</script>

<Loading loadable={project.current}>
	{#snippet children(project)}
		<button
			type="button"
			class="project-btn"
			class:current={focused}
			onclick={() => {
				goto(routes.projectReviewPath({ ownerSlug: project.owner, projectSlug: project.slug }));
			}}
		>
			<div class="pip"></div>
			<div class="link-container">
				<p class="text-13">{showOwner ? `${project.owner}/${project.slug}` : `${project.slug}`}</p>
				<div class="icon">
					<Icon name="chevron-right"></Icon>
				</div>
			</div>
		</button>
	{/snippet}
</Loading>

<style lang="postcss">
	.project-btn {
		display: flex;
		align-items: center;
		width: 100%;

		gap: 9px;
		cursor: pointer;

		&.current {
			.pip {
				background-color: var(--clr-core-pop-50);
			}

			.link-container {
				background-color: var(--clr-theme-pop-bg-muted);
			}

			.icon {
				display: block;
			}
		}
	}

	.pip {
		width: 10px;
		height: 18px;
		margin-left: -5px;
		border-radius: 5px;
	}

	.link-container {
		display: flex;
		flex-grow: 1;
		align-items: center;
		justify-content: space-between;
		margin-right: 14px;

		padding: 10px 14px;
		gap: 10px;

		border-radius: var(--radius-m);
	}

	.icon {
		display: none;
		height: 16px;
		margin-right: -6px;
	}
</style>
