<script lang="ts">
	import emptyFolderSvg from '$lib/assets/empty-state/empty-folder.svg?raw';
	import mcpLogoSvg from '$lib/assets/unsized-logos/mcp.svg?raw';
	import {
		CardGroup,
		EmptyStatePlaceholder,
		Link,
		Modal,
		ScrollableContainer,
		Toggle
	} from '@gitbutler/ui';
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

<Modal
	bind:this={modal}
	width={480}
	title={Object.entries(mcpConfig.mcpServers).length > 0 ? 'MCP server configuration' : undefined}
>
	<ScrollableContainer>
		<div class="flex flex-col gap-8">
			{#if Object.entries(mcpConfig.mcpServers).length === 0}
				<EmptyStatePlaceholder image={emptyFolderSvg} width={300} topBottomPadding={38}>
					{#snippet title()}
						No MCP servers available
					{/snippet}
					{#snippet caption()}
						For installing additional MCP servers,<br />check the
						<Link href="https://docs.anthropic.com/en/docs/claude-code/mcp#installing-mcp-servers"
							>Claude Code documentation</Link
						>
					{/snippet}
				</EmptyStatePlaceholder>
			{:else}
				<p class="text-12 text-body clr-text-2 m-b-10">
					Select the MCP Servers for this session. For installing additional MCP servers, check the
					<Link href="https://docs.anthropic.com/en/docs/claude-code/mcp#installing-mcp-servers"
						>Claude Code documentation</Link
					>
				</p>
				<CardGroup>
					{#each Object.entries(mcpConfig.mcpServers) as [name, server]}
						{@render mcpServer(name, server)}
					{/each}
				</CardGroup>
			{/if}
		</div>
	</ScrollableContainer>
</Modal>

{#snippet mcpServer(name: string, server: McpServer)}
	<CardGroup.Item labelFor={name}>
		{#snippet iconSide()}
			<div class="mcp-server__icon">
				{@html mcpLogoSvg}
			</div>
		{/snippet}

		{#snippet title()}
			<p class="text-14 text-bold">
				{name}
			</p>
		{/snippet}

		{#snippet caption()}
			{#if server.command}
				<span>{server.command} {server.args?.join(' ')}</span>
			{/if}
			{#if server.url}
				<span>{server.url}</span>
			{/if}
		{/snippet}

		{#snippet actions()}
			<Toggle
				id={name}
				checked={!disabledServers.includes(name)}
				onclick={() => toggleServer(name)}
			/>
		{/snippet}
	</CardGroup.Item>
{/snippet}

<style lang="postcss">
	.mcp-server__icon {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		padding: 6px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
	}
</style>
