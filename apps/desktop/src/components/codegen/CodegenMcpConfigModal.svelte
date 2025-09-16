<script lang="ts">
	import { Link, Modal, ScrollableContainer, Toggle } from '@gitbutler/ui';
	import type { McpConfig, McpServer } from '$lib/codegen/types';

	type Props = {
		mcpConfig: McpConfig;
		disabledServers: string[];
		toggleServer: (server: string) => void;
	};

	const { mcpConfig, disabledServers, toggleServer }: Props = $props();

	let modal = $state<Modal>();

	export function open() {
		modal?.show();
	}
</script>

<Modal bind:this={modal} title="Configure your MCP servers">
	<ScrollableContainer>
		<div class="flex flex-col gap-12">
			<div>
				<p class="text-13">
					Choose which MCP Servers should be available in this session. To install more MCP servers,
					please refer to the
					<Link href="https://docs.anthropic.com/en/docs/claude-code/mcp#installing-mcp-servers"
						>Claude Code documentation</Link
					>
				</p>
			</div>
			{#if Object.entries(mcpConfig.mcpServers).length === 0}
				<p class="text-13">You currently have no MCP Servers available to this project.</p>
			{:else}
				{#each Object.entries(mcpConfig.mcpServers) as [name, server]}
					{@render mcpServer(name, server)}
				{/each}
			{/if}
		</div>
	</ScrollableContainer>
</Modal>

{#snippet mcpServer(name: string, server: McpServer)}
	<div class="mcp-server">
		<div class="mcp-server__body">
			<p class="text-14 text-bold">{name}</p>
			<p class="text-13">{server.command} {server.args?.join(' ')}</p>
		</div>
		<div class="mcp-server__actions">
			<Toggle checked={!disabledServers.includes(name)} onclick={() => toggleServer(name)} />
		</div>
	</div>
{/snippet}

<style lang="postcss">
	.mcp-server {
		display: flex;
		padding: 12px;

		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}
	.mcp-server__body {
		display: flex;

		flex-grow: 1;

		flex-direction: column;
		gap: 8px;
	}
</style>
