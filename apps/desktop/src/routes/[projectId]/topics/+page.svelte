<script lang="ts">
	import { getGitHost } from '$lib/forge/interface/forge';
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
	import CreateIssueModal from '$lib/topics/CreateIssueModal.svelte';
	import CreateTopicModal from '$lib/topics/CreateTopicModal.svelte';
	import Topic from '$lib/topics/Topic.svelte';
	import { TopicService } from '$lib/topics/service';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	const topicService = getContext(TopicService);
	const topics = topicService.topics;
	const gitHost = getGitHost();

	const sortedTopics = $derived.by(() => {
		const clonedTopics = structuredClone($topics);
		clonedTopics.sort((a, b) => b.createdAt - a.createdAt);

		return clonedTopics;
	});

	let createTopicModal = $state<ReturnType<typeof CreateTopicModal>>();
	let createIssueModal = $state<ReturnType<typeof CreateIssueModal>>();
</script>

<CreateTopicModal bind:this={createTopicModal} />
<CreateIssueModal bind:this={createIssueModal} />

<SettingsPage title="Topics">
	<div>
		<div class="topic__actions">
			<Button kind="solid" style="pop" onclick={() => createTopicModal?.open()}>Create Topic</Button
			>
			{#if $gitHost?.issueService()}
				<Button style="pop" onclick={() => createIssueModal?.open()}>Create Issue</Button>
			{/if}
		</div>
		{#if sortedTopics.length > 0}
			<div class="container">
				{#each sortedTopics as topic}
					<Topic {topic} />
				{/each}
			</div>
		{/if}
	</div>
</SettingsPage>

<style lang="postcss">
	.container {
		width: 100%;

		background-color: var(--clr-bg-1);
		border-radius: var(--radius-l);

		border: 1px solid var(--clr-border-2);
	}

	.topic__actions {
		display: flex;

		gap: 8px;

		margin-bottom: 16px;
	}
</style>
