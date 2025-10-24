<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import { sessionMessage, type ClaudeSessionDetails } from '$lib/codegen/types';
	import { inject } from '@gitbutler/core/context';
	import type { Snippet } from 'svelte';

	type Props = {
		projectId: string;
		sessionId: string;
		fallback?: Snippet;
		loading?: Snippet;
		error?: Snippet;
		children: Snippet<[string]>;
	};

	const { projectId, sessionId, children, fallback, loading, error }: Props = $props();
	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const sessionDetails = $derived(claudeCodeService.sessionDetails(projectId, sessionId));
</script>

{#snippet loadingSnippet()}
	{#if loading}
		{@render loading()}
	{:else if fallback}
		{@render fallback()}
	{/if}
{/snippet}

{#snippet errorSnippet()}
	{#if error}
		{@render error()}
	{:else if fallback}
		{@render fallback()}
	{/if}
{/snippet}

{#snippet resultChildren(sessionDetails: ClaudeSessionDetails)}
	{@const title = sessionMessage(sessionDetails) ?? sessionId}
	{@render children(title)}
{/snippet}

<ReduxResult
	{projectId}
	result={sessionDetails.result}
	loading={loadingSnippet}
	error={errorSnippet}
	children={resultChildren}
></ReduxResult>
