<!--
	Compound child that owns MCP config and renders CodegenMessages for the stack details panel.
	Reads shared state from StackController via context.

	Usage:
	```svelte
	<StackCodegen {hasRulesToClear} {claudeConfig} onclose={() => controller.closePreview()} />
	```
-->
<script lang="ts">
	import CodegenMcpConfigModal from "$components/codegen/CodegenMcpConfigModal.svelte";
	import CodegenMessages from "$components/codegen/CodegenMessages.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { CLAUDE_CODE_SERVICE } from "$lib/codegen/claude";
	import { getStackContext } from "$lib/stacks/stackController.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import type { ClaudeConfig } from "$lib/codegen/types";

	type Props = {
		hasRulesToClear: boolean;
		claudeConfig: ClaudeConfig;
		onclose: () => void;
	};

	const { hasRulesToClear, claudeConfig, onclose }: Props = $props();

	const controller = getStackContext();
	const uiState = inject(UI_STATE);
	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);

	let mcpConfigModal = $state<CodegenMcpConfigModal>();
	const mcpClaudeConfigQuery = $derived(
		claudeCodeService.claudeConfig({ projectId: controller.projectId }),
	);
</script>

<CodegenMessages
	{onclose}
	onMcpSettings={() => mcpConfigModal?.open()}
	{hasRulesToClear}
	projectRegistered={claudeConfig.projectRegistered}
/>

<!-- MCP CONFIG MODAL -->
<ReduxResult
	result={mcpClaudeConfigQuery.result}
	projectId={controller.projectId}
	stackId={controller.stackId}
	hideError
>
	{#snippet children(config, { stackId: resolvedStackId })}
		{@const resolvedLaneState = resolvedStackId ? uiState.lane(resolvedStackId) : undefined}
		<CodegenMcpConfigModal
			disabledServers={resolvedLaneState?.disabledMcpServers.current || []}
			bind:this={mcpConfigModal}
			claudeConfig={config}
			toggleServer={(server) => {
				const disabledServers = resolvedLaneState?.disabledMcpServers.current;
				if (disabledServers) {
					if (disabledServers.includes(server)) {
						resolvedLaneState?.disabledMcpServers.set(disabledServers.filter((s) => s !== server));
					} else {
						resolvedLaneState?.disabledMcpServers.set([...disabledServers, server]);
					}
				}
			}}
		/>
	{/snippet}
</ReduxResult>
