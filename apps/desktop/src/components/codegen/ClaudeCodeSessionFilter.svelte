<script lang="ts">
	import ClaudeSessionTitle from "$components/codegen/ClaudeSessionTitle.svelte";
	import { Codeblock } from "@gitbutler/ui";

	type Props = {
		projectId: string;
		sessionId: string;
	};

	const { projectId, sessionId }: Props = $props();

	const claudeResumeCommand = $derived(`claude --resume ${sessionId}`);
</script>

<div class="cc-session-filter">
	<ClaudeSessionTitle {projectId} {sessionId}>
		{#snippet children(descriptor)}
			<p class="descriptor text-14 text-body text-bold">
				{#if descriptor}
					{descriptor}
				{:else}
					<span>Id: {sessionId}</span>
				{/if}
			</p>
		{/snippet}
	</ClaudeSessionTitle>
</div>

<Codeblock label="Resume session command" content={claudeResumeCommand} />

<style lang="postcss">
	.cc-session-filter {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		margin-bottom: 12px;
		gap: 8px;
	}

	.descriptor {
		display: -webkit-box;
		-webkit-line-clamp: 3;
		-webkit-box-orient: vertical;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
