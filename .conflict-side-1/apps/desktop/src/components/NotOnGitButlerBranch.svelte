<script lang="ts">
	import Chrome from '$components/Chrome.svelte';
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import ProjectNameLabel from '$components/ProjectNameLabel.svelte';
	import ProjectSwitcher from '$components/ProjectSwitcher.svelte';
	import RemoveProjectButton from '$components/RemoveProjectButton.svelte';
	import derectionDoubtSvg from '$lib/assets/illustrations/direction-doubt.svg?raw';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { VirtualBranchService } from '$lib/branches/virtualBranchService';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { showError } from '$lib/notifications/toasts';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { getContext } from '@gitbutler/shared/context';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import * as toasts from '@gitbutler/ui/toasts';
	import type { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { goto } from '$app/navigation';

	interface Props {
		baseBranch: BaseBranch;
	}

	const { baseBranch }: Props = $props();

	const projectsService = getContext(ProjectsService);
	const vbranchService = getContext(VirtualBranchService);
	const baseBranchService = getContext(BaseBranchService);
	const project = getContext(Project);
	const [setBaseBranchTarget, targetBranchSwitch] = baseBranchService.setTarget;

	const settingsService = getContext(SettingsService);
	const appSettings = settingsService.appSettings;

	async function switchTarget(branch: string, remote?: string) {
		await setBaseBranchTarget({
			projectId: project.id,
			branch,
			pushRemote: remote
		});
		await vbranchService.refresh();
	}

	let isDeleting = $state(false);
	let deleteConfirmationModal: ReturnType<typeof RemoveProjectButton> | undefined = $state();

	async function onDeleteClicked() {
		if (project) {
			isDeleting = true;
			try {
				deleteConfirmationModal?.close();
				await projectsService.deleteProject(project.id);
				toasts.success('Project deleted');
				goto('/', { invalidateAll: true });
			} catch (err: any) {
				console.error(err);
				showError('Failed to delete project', err);
			} finally {
				isDeleting = false;
				projectsService.reload();
			}
		}
	}
</script>

{#snippet page()}
	<DecorativeSplitView img={derectionDoubtSvg}>
		<div class="switchrepo">
			<div class="project-name">
				<ProjectNameLabel projectName={project?.title} />
			</div>
			<p class="switchrepo__title text-18 text-body text-bold">
				Looks like you've switched away from <span class="code-string"> gitbutler/workspace </span>
			</p>

			<p class="switchrepo__message text-13 text-body">
				Due to GitButler managing multiple virtual branches, you cannot switch back and forth
				between git branches and virtual branches easily.
				<Link href="https://docs.gitbutler.com/features/virtual-branches/integration-branch">
					Learn more
				</Link>
			</p>

			<div class="switchrepo__actions">
				<AsyncButton
					style="pop"
					icon="undo-small"
					reversedDirection
					loading={targetBranchSwitch.current.isLoading}
					action={async () => {
						if (baseBranch) {
							await switchTarget(baseBranch.branchName);
						}
					}}
				>
					Go back to gitbutler/workspace
				</AsyncButton>

				{#if project}
					<RemoveProjectButton
						bind:this={deleteConfirmationModal}
						projectTitle={project.title}
						{isDeleting}
						{onDeleteClicked}
					/>
				{/if}
			</div>

			<Spacer dotted margin={0} />

			<div class="switchrepo__project">
				<ProjectSwitcher />
			</div>
		</div>
	</DecorativeSplitView>
{/snippet}

{#if $appSettings?.featureFlags.v3}
	<Chrome projectId={project.id} sidebarDisabled>
		{@render page()}
	</Chrome>
{:else}
	{@render page()}
{/if}

<style lang="postcss">
	.project-name {
		margin-bottom: 12px;
	}

	.switchrepo__title {
		color: var(--clr-scale-ntrl-30);
		margin-bottom: 12px;
	}

	.switchrepo__message {
		color: var(--clr-scale-ntrl-50);
		margin-bottom: 20px;
	}
	.switchrepo__actions {
		display: flex;
		gap: 8px;
		padding-bottom: 24px;
		flex-wrap: wrap;
	}

	.switchrepo__project {
		padding-top: 24px;
	}
</style>
