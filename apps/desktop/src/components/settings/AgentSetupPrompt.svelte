<script lang="ts">
	import { AGENTS_SERVICE } from "$lib/agents/agentsService";
	import { markAgentPromptEvaluated } from "$lib/agents/setupPrompt";
	import { BACKEND } from "$lib/backend";
	import { CLI_MANAGER } from "$lib/config/cli";
	import { showToast } from "$lib/notifications/toasts";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { onMount } from "svelte";
	import type { AgentFramework } from "@gitbutler/but-sdk";

	// Injection must happen during component init, not inside onMount.
	const backend = inject(BACKEND);
	const settingsService = inject(SETTINGS_SERVICE);
	const cliManager = inject(CLI_MANAGER);
	const agentsService = inject(AGENTS_SERVICE);
	const uiState = inject(UI_STATE);

	onMount(async () => {
		if (!markAgentPromptEvaluated()) return;
		if (backend.platformName === "web") return;

		try {
			const settings = await settingsService.fetchAppSettings();
			// Don't interrupt first-run onboarding, and never re-ask.
			if (!settings.onboardingComplete) return;
			if (settings.agents?.skillsPromptDismissed) return;

			// Promise-based reads, not useQuery: this component renders nothing,
			// so a subscription would never be released and an effect would fire
			// once while still loading and again with data.
			const [cli, status] = await Promise.all([
				cliManager.fetchState(),
				agentsService.fetchStatus({}),
			]);

			const cliMissing = !cli.installed && !cli.blockedReason;
			const noSkills = status.frameworks.every((framework: AgentFramework) =>
				framework.scopes.every((entry) => !entry.installed),
			);
			if (!cliMissing && !noSkills) return;

			showToast({
				style: "info",
				title: "Set up GitButler for your coding agent",
				message:
					"Install the `but` CLI and the GitButler skill so your agent can branch, commit, and open pull requests the way you want.",
				dismissLabel: "Don't ask again",
				extraAction: {
					label: "Enable agent skills",
					onClick: (dismiss) => {
						uiState.global.modal.set({ type: "agent-setup" });
						dismiss();
					},
				},
				onDismiss: async () => {
					await settingsService.updateAgents({ skillsPromptDismissed: true });
				},
			});
		} catch (err: unknown) {
			// A nudge is never worth blocking startup over.
			console.error("Agent setup prompt check failed", err);
		}
	});
</script>
