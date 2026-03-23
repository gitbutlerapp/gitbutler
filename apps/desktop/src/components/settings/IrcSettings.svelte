<script lang="ts">
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { USER } from "$lib/user/user";
	import { inject } from "@gitbutler/core/context";
	import { Badge, Button, CardGroup, Textbox, Toggle } from "@gitbutler/ui";
	import type { IconName } from "@gitbutler/ui";
	import type { ComponentColorType } from "@gitbutler/ui/utils/colorTypes";

	const settingsService = inject(SETTINGS_SERVICE);
	const user = inject(USER);
	const ircApiService = inject(IRC_API_SERVICE);

	const settings = settingsService.appSettings;

	const irc = $derived($settings?.irc);

	function connectionBadge(state: string | undefined): {
		style: ComponentColorType;
		icon: IconName | undefined;
		label: string;
	} {
		switch (state) {
			case "ready":
				return { style: "safe", icon: "tick-circle", label: "Connected" };
			case "connecting":
				return { style: "warning", icon: "spinner", label: "Connecting" };
			case "negotiating":
				return { style: "warning", icon: "spinner", label: "Negotiating" };
			case "reconnecting":
				return { style: "warning", icon: "spinner", label: "Reconnecting" };
			case "error":
				return { style: "danger", icon: "danger", label: "Error" };
			case "disconnected":
			default:
				return { style: "gray", icon: undefined, label: "Disconnected" };
		}
	}

	const stateQuery = $derived(ircApiService.connectionState());
	const status = $derived(connectionBadge(stateQuery?.response?.state));

	const connected = $derived(
		stateQuery?.response?.state !== "disconnected" && stateQuery?.response?.state !== undefined,
	);

	async function disconnect() {
		await ircApiService.disconnect();
		await settingsService.updateIrc({ connection: { enabled: false } });
	}
</script>

{#if irc}
	<p class="text-12 text-body irc-settings__text">
		Configure IRC integration for remote collaboration on Claude Code sessions.
	</p>

	<CardGroup>
		<CardGroup.Item>
			<div class="server-config">
				<Textbox
					value={irc.server.host}
					size="large"
					label="Server Host"
					placeholder="irc.gitbutler.com"
					onchange={(value) => settingsService.updateIrc({ server: { host: value } })}
				/>
			</div>
		</CardGroup.Item>

		<CardGroup.Item labelFor="auto-share">
			{#snippet title()}
				Auto-share new sessions
			{/snippet}
			{#snippet caption()}
				Automatically share new Claude Code sessions to IRC when connected.
			{/snippet}
			{#snippet actions()}
				<Toggle
					id="auto-share"
					checked={irc.autoShare}
					onclick={() => settingsService.updateIrc({ autoShare: !irc.autoShare })}
				/>
			{/snippet}
		</CardGroup.Item>
	</CardGroup>

	<CardGroup>
		<CardGroup.Item labelFor="irc-enabled">
			{#snippet title()}
				<span class="enable-row">
					Connect
					{#if irc.connection.enabled}
						<Badge style={status.style} kind="soft" size="tag" icon={status.icon}>
							{status.label}
						</Badge>
						{#if connected}
							<Button size="tag" kind="outline" onclick={disconnect}>Disconnect</Button>
						{/if}
					{/if}
				</span>
			{/snippet}
			{#snippet actions()}
				<Toggle
					id="irc-enabled"
					checked={irc.connection.enabled}
					onclick={() =>
						settingsService.updateIrc({
							connection: { enabled: !irc.connection.enabled },
						})}
				/>
			{/snippet}
		</CardGroup.Item>

		<CardGroup.Item>
			<Textbox
				value={irc.connection.nickname ?? ""}
				size="large"
				label="Nickname"
				placeholder={$user?.login ?? "your-nickname"}
				onchange={(value) => settingsService.updateIrc({ connection: { nickname: value || null } })}
			/>
		</CardGroup.Item>

		<CardGroup.Item>
			<Textbox
				value={irc.connection.serverPassword ?? ""}
				size="large"
				type="password"
				label="Server Password"
				placeholder="Shared connection password"
				onchange={(value) =>
					settingsService.updateIrc({ connection: { serverPassword: value || null } })}
			/>
			<p class="text-11 text-body caption-text">
				Stored in plaintext. Use the password you were given.
			</p>
		</CardGroup.Item>

		<CardGroup.Item>
			<Textbox
				value={irc.connection.saslPassword ?? ""}
				size="large"
				type="password"
				label="Account Password"
				placeholder="Your account password"
				onchange={(value) =>
					settingsService.updateIrc({ connection: { saslPassword: value || null } })}
			/>
			<p class="text-11 text-body caption-text">
				Stored in plaintext — do not reuse a password from another service. Used for SASL
				authentication. On first connect this registers your account with this password.
			</p>
		</CardGroup.Item>

		<CardGroup.Item>
			<Textbox
				value={irc.connection.realname ?? ""}
				size="large"
				label="Real Name"
				placeholder={$user?.name ?? $user?.login ?? "Your Name"}
				onchange={(value) => settingsService.updateIrc({ connection: { realname: value || null } })}
			/>
		</CardGroup.Item>

		<CardGroup.Item>
			<Textbox
				value={irc.projectChannel ?? ""}
				size="large"
				label="Project channel"
				placeholder="#<project-name> (auto)"
				onchange={(value) => settingsService.updateIrc({ projectChannel: value || null })}
			/>
			<p class="text-11 text-body caption-text">
				Channel to join when opening a project. Leave empty to auto-derive
				<code>#&lt;project-name&gt;</code>.
			</p>
		</CardGroup.Item>
	</CardGroup>
{/if}

<style>
	.irc-settings__text {
		margin-bottom: 10px;
		color: var(--clr-text-2);
	}

	.enable-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.server-config {
		display: grid;
		grid-template-columns: 1fr auto;
		width: 100%;
		gap: 12px;
	}

	.caption-text {
		margin-top: 6px;
		color: var(--clr-text-3);
	}

	code {
		padding: 2px 6px;
		border-radius: 3px;
		background-color: var(--clr-bg-2);
		font-size: 11px;
		font-family: var(--font-mono);
	}
</style>
