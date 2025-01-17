<script lang="ts">
	import { ProjectService } from '$lib/project/projectService';
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

	const fileTypes = ['image/jpeg', 'image/png'];

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

	let pictureObjectUrl = $state<string>();
	let picture = $state<File>();

	function onPictureChange(e: Event) {
		const target = e.target as HTMLInputElement;
		const file = target.files?.[0];

		if (file && fileTypes.includes(file.type)) {
			pictureObjectUrl = URL.createObjectURL(file);
			picture = file;
		}
	}

	function onDrop(event: DragEvent) {
		event.preventDefault();

		draggingPicture = false;
		if (!event.dataTransfer?.items) return;

		const file = [...(event.dataTransfer?.items || [])].find((item) =>
			fileTypes.includes(item.type)
		);
		if (!file) return;

		const fileObject = file.getAsFile() || undefined;
		if (!fileObject) return;

		pictureObjectUrl = URL.createObjectURL(fileObject);
		picture = fileObject;
	}

	// Post creation
	let newPostContent = $derived(persisted('', `postContent--${feedIdentity}--${replyTo}`));
	function createPost() {
		if (feedIdentity?.current.status !== 'found') return;
		if (parentProject?.current?.status !== 'found') return;

		feedService.createPost(
			$newPostContent,
			parentProject.current.value.repositoryId,
			feedIdentity.current.value,
			replyTo,
			picture
		);

		$newPostContent = '';
		pictureObjectUrl = undefined;
		picture = undefined;
	}

	let pictureInput = $state<HTMLElement>();
	let dragTarget = $state<HTMLElement>();
	let draggingPicture = $state(false);

	function onDragStart() {
		draggingPicture = true;
	}
	function onDragEnd() {
		draggingPicture = false;
	}

	$effect(() => {
		if (!dragTarget) return;

		dragTarget.addEventListener('dragenter', onDragStart);
		dragTarget.addEventListener('dragleave', onDragEnd);

		return () => {
			if (!dragTarget) return;
			dragTarget.removeEventListener('dragenter', onDragStart);
			dragTarget.removeEventListener('dragleave', onDragEnd);
		};
	});
</script>

<SectionCard>
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div bind:this={dragTarget} class:dragging={draggingPicture} ondrop={onDrop}>
		<Textarea
			bind:value={$newPostContent}
			placeholder="What is going on?"
			class="create-post-textarea-transparent"
		/>
	</div>

	<input
		bind:this={pictureInput}
		onchange={onPictureChange}
		type="file"
		id="picture"
		name="picture"
		accept={fileTypes.join(',')}
		class="hidden-input"
	/>

	{#if pictureObjectUrl}
		<hr />
		<div class="create-bleep-container"></div>

		<label id="profile-picture" class="profile-pic-wrapper focus-state" for="picture">
			{#if pictureObjectUrl}
				<img src={pictureObjectUrl} alt="" referrerpolicy="no-referrer" />
			{/if}
		</label>
	{/if}

	<div class="create-bleep-container">
		{#if pictureObjectUrl}
			<Button kind="outline" onclick={() => (pictureObjectUrl = undefined)}>Remove picture</Button>
		{:else}
			<Button kind="outline" onclick={() => pictureInput?.click()}>🖼️</Button>
		{/if}
		<Button onclick={createPost}>Create Bleep 🚀</Button>
	</div>
</SectionCard>

<style lang="postcss">
	.create-bleep-container {
		display: flex;
		justify-content: flex-end;

		gap: 8px;
	}

	.hidden-input {
		display: none;
	}

	.dragging {
		border: 2px dashed var(--clr-scale-pop-50);

		background-color: var(--clr-scale-pop-90);
	}

	:global(.create-post-textarea-transparent) {
		background: transparent;
	}
</style>
