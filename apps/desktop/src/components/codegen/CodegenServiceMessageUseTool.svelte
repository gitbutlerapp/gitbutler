<script lang="ts">
	import CodegenServiceMessage from '$components/codegen/CodegenServiceMessage.svelte';
	import type { ToolCall } from '$lib/codegen/messages';

	type Props = { toolCall: ToolCall };

	const { toolCall }: Props = $props();

	const wantsToDo = $derived.by(() => {
		switch (toolCall.name) {
			case 'Task':
				return 'start a subagent';
			case 'Bash':
				return 'run a command';
			case 'Glob':
				return 'pattern match some files';
			case 'Grep':
				return 'perform a content search';
			case 'Read':
				return 'read a file';
			case 'Edit':
				return 'edit a file';
			case 'MultiEdit':
				return 'edit a file';
			case 'Write':
				return 'create a file';
			case 'WebFetch':
				return 'search the internet';
			case 'WebSearch':
				return 'search the internet';
			default:
				return 'do something requiring your approval';
		}
	});

	const actionName = $derived.by(() => {
		switch (toolCall.name) {
			case 'Task':
				return 'subagent request';
			case 'Bash':
				return 'command';
			case 'Glob':
				return 'pattern match';
			case 'Grep':
				return 'content search';
			case 'Read':
				return 'read request';
			case 'Edit':
				return 'edit request';
			case 'MultiEdit':
				return 'edit request';
			case 'Write':
				return 'create request';
			case 'WebFetch':
				return 'internet search';
			case 'WebSearch':
				return 'internet search';
			default:
				return 'action';
		}
	});
</script>

<CodegenServiceMessage style="pop" face="waiting">
	<div class="flex flex-col gap-2">
		<p class="text-13 text-semibold text-body">Claude Code wants to {wantsToDo} 👆</p>
		<p class="text-13 text-italic text-body opacity-60">
			Review the {actionName} above, then approve or reject.
		</p>
	</div>
</CodegenServiceMessage>
