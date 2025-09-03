<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import { sessionMessage, type ClaudeSessionDetails } from '$lib/codegen/types';
	import { inject } from '@gitbutler/core/context';
	import type { Snippet } from 'svelte';

	type Props = {
		projectId: string;
		sessionId: string;
		fallback: Snippet;
		children: Snippet<[string]>;
	};

	const { projectId, sessionId, children, fallback }: Props = $props();
	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const sessionDetails = $derived(claudeCodeService.sessionDetails(projectId, sessionId));
</script>

{#snippet loading()}
	{@render fallback()}
{/snippet}

{#snippet error()}
	{@render fallback()}
{/snippet}

{#snippet resultChildren(sessionDetails: ClaudeSessionDetails)}
	{@const title = sessionMessage(sessionDetails) ?? sessionId}
	{@render children(title)}
{/snippet}

<ReduxResult {projectId} result={sessionDetails.current} {loading} {error} children={resultChildren}
></ReduxResult>
