<script lang="ts">
	import emptyFolderSvg from '$lib/assets/empty-state/empty-folder.svg?raw';
	import mcpLogoSvg from '$lib/assets/unsized-logos/mcp.svg?raw';
	import {
		Link,
		Modal,
		ScrollableContainer,
		Toggle,
		SectionCard,
		EmptyStatePlaceholder
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
	title={Object.entries(mcpConfig.mcpServers).length > 0 ? 'MCP Server Configuration' : undefined}
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
				<p class="text-12 text-body clr-text-2 m-bottom-10">
					Select the MCP Servers for this session. For installing additional MCP servers, check the
					<Link href="https://docs.anthropic.com/en/docs/claude-code/mcp#installing-mcp-servers"
						>Claude Code documentation</Link
					>
				</p>
				<div class="stack-v">
					{#each Object.entries(mcpConfig.mcpServers) as [name, server], index}
						{@render mcpServer(
							name,
							server,
							index === 0,
							index === Object.entries(mcpConfig.mcpServers).length - 1
						)}
					{/each}
				</div>
			{/if}
		</div>
	</ScrollableContainer>
</Modal>

{#snippet mcpServer(name: string, server: McpServer, isFirst: boolean, isLast: boolean)}
	<SectionCard orientation="row" labelFor={name} roundedTop={isFirst} roundedBottom={isLast}>
		{#snippet iconSide()}
			<div class="mcp-server__icon">
				{@html mcpLogoSvg}
			</div>
		{/snippet}
		<div class="mcp-server__body">
			<p class="text-14 text-bold">
				{name}
			</p>

			<p class="mcp-server__caption truncate">
				{#if server.command}
					<span>{server.command} {server.args?.join(' ')}</span>
				{/if}
				{#if server.url}
					<span>{server.url}</span>
				{/if}
			</p>
		</div>
		{#snippet actions()}
			<Toggle
				id={name}
				checked={!disabledServers.includes(name)}
				onclick={() => toggleServer(name)}
			/>
		{/snippet}
	</SectionCard>
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
	.mcp-server__body {
		display: flex;
		flex-direction: column;
		width: 100%;
		overflow: hidden;
		gap: 2px;
	}
	.mcp-server__caption {
		color: var(--clr-text-2);
		font-size: 11px;
		font-family: var(--fontfamily-mono);
	}
</style>
