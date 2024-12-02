<script lang="ts">
	import { ProjectService } from '$lib/backend/projects';
	import { getContext } from '@gitbutler/shared/context';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
	import {
		getFeedIdentityForRepositoryId,
		getParentForRepositoryId
	} from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { persisted } from '@gitbutler/shared/persisted';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';

	type Props = {
		replyTo?: string;
	};

	const { replyTo }: Props = $props();

	const appState = getContext(AppState);
	const feedService = getContext(FeedService);
	const projectService = getContext(ProjectService);
	const project = projectService.project;
	const cloudProjectService = getContext(CloudProjectService);

	const parentProject = $derived(
		$project?.api
			? getParentForRepositoryId(appState, cloudProjectService, $project.api.repository_id)
			: undefined
	);

	const feedIdentity = $derived(
		$project?.api
			? getFeedIdentityForRepositoryId(appState, cloudProjectService, $project.api.repository_id)
			: undefined
	);

	// Post creation
	let newPostContent = $derived(persisted('', `postContent--${feedIdentity}--${replyTo}`));
	function createPost() {
		if (!feedIdentity?.current) return;
		if (!parentProject?.current) return;

		feedService.createPost(
			$newPostContent,
			parentProject.current.repositoryId,
			feedIdentity.current,
			replyTo
		);
		$newPostContent = '';
	}
</script>

<SectionCard>
	<Textarea bind:value={$newPostContent} placeholder="What is going on?" />
	<div class="create-bleep-container">
		<Button onclick={createPost}>Create Bleep 🚀</Button>
	</div>
</SectionCard>

<style lang="postcss">
	.create-bleep-container {
		display: flex;
		justify-content: flex-end;
	}
</style>
