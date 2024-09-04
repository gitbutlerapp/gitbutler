<script lang="ts">
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import ProjectSwitcher from './ProjectSwitcher.svelte';
	import RemoveProjectButton from './RemoveProjectButton.svelte';
	import notFoundSvg from '$lib/assets/illustrations/not-found.svg?raw';
	import { ProjectService } from '$lib/backend/projects';
	import InfoMessage from '$lib/shared/InfoMessage.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import { getContext } from '$lib/utils/context';
	import Button from '@gitbutler/ui/Button.svelte';

	const projectService = getContext(ProjectService);
	const id = projectService.getLastOpenedProject();
	const projectPromise = id
		? projectService.getProject(id, true)
		: Promise.reject('Failed to get project');

	let deleteSucceeded: boolean | undefined = $state(undefined);
	let isDeleting = $state(false);

	async function stopTracking(id: string) {
		isDeleting = true;
		deleteProject: {
			try {
				await projectService.deleteProject(id);
			} catch (e) {
				deleteSucceeded = false;
				break deleteProject;
			}
			deleteSucceeded = true;
		}
		isDeleting = false;
	}

	async function locate(id: string) {
		await projectService.relocateProject(id);
	}

	function getDeletionStatusMessage(repoName: string) {
		if (deleteSucceeded === undefined) return null;
		if (deleteSucceeded) return `Project "${repoName}" successfully deleted`;
		return `Failed to delete "${repoName}" project`;
	}
</script>

<DecorativeSplitView img={notFoundSvg}>
	<div class="container" data-tauri-drag-region>
		{#if deleteSucceeded === undefined}
			{#await projectPromise then project}
				<div class="text-content">
					<h2 class="title-text text-18 text-body text-bold" data-tauri-drag-region>
						Can’t find "{project.title}"
					</h2>

					<p class="description-text text-13 text-body">
						Sorry, we can't find the project you're looking for.
						<br />
						It might have been removed or doesn't exist.
						<button class="check-again-btn" onclick={() => location.reload()}>Click here</button>
						to check again.
						<br />
						The current project path: <span class="code-string">{project.path}</span>
					</p>
				</div>

				<div class="button-container">
					<Button
						type="button"
						style="pop"
						kind="solid"
						onclick={async () => await locate(project.id)}>Locate project…</Button
					>
					<RemoveProjectButton
						noModal
						{isDeleting}
						onDeleteClicked={async () => await stopTracking(project.id)}
					/>
				</div>

				{#if deleteSucceeded !== undefined}
					<InfoMessage filled outlined={false} style="success" icon="info">
						<svelte:fragment slot="content"
							>{getDeletionStatusMessage(project.title)}</svelte:fragment
						>
					</InfoMessage>
				{/if}
			{:catch}
				<div class="text-content">
					<h2 class="title-text text-18 text-body text-bold">Can’t find project</h2>
				</div>
			{/await}
		{/if}

		<Spacer dotted margin={0} />
		<ProjectSwitcher />
	</div>
</DecorativeSplitView>

<style lang="postcss">
	.container {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.button-container {
		display: flex;
		gap: 8px;
	}

	.text-content {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.title-text {
		color: var(--clr-scale-ntrl-30);
		/* margin-bottom: 12px; */
	}

	.description-text {
		color: var(--clr-text-2);
		line-height: 1.6;
	}

	.check-again-btn {
		text-decoration: underline;
	}
</style>
