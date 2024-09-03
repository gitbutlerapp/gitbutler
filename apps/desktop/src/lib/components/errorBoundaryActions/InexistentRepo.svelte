<script lang="ts">
	import RemoveProjectButton from '../RemoveProjectButton.svelte';
	import { ProjectService } from '$lib/backend/projects';
	import { getContext } from '$lib/utils/context';
	import Button from '@gitbutler/ui/Button.svelte';

	interface Props {
		ondeletesuccess: (deleted: boolean) => void;
	}

	const { ondeletesuccess }: Props = $props();

	const projectService = getContext(ProjectService);

	let isDeleting = $state(false);

	async function stopTracking() {
		isDeleting = true;
		deleteProject: {
			const id = projectService.getLastOpenedProject();
			if (id === undefined) {
				ondeletesuccess(false);
				break deleteProject;
			}

			try {
				await projectService.deleteProject(id);
			} catch (e) {
				ondeletesuccess(false);
				break deleteProject;
			}

			ondeletesuccess(true);
		}
		isDeleting = false;
	}

	async function locate() {
		const id = projectService.getLastOpenedProject();
		if (id === undefined) {
			return;
		}

		await projectService.relocateProject(id);
	}
</script>

<div class="button-container">
	<Button type="button" style="pop" kind="solid" onclick={locate}>Locate project…</Button>
	<RemoveProjectButton noModal {isDeleting} onDeleteClicked={stopTracking} />
</div>

<style lang="postcss">
	.button-container {
		display: flex;
		gap: 8px;
	}
</style>
