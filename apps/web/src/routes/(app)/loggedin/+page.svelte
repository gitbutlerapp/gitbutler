<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import FullscreenUtilityCard from "$lib/components/service/FullscreenUtilityCard.svelte";
	import { inject } from "@gitbutler/core/context";
	import { LOGIN_SERVICE } from "@gitbutler/shared/login/loginService";
	import { AsyncButton, Button, chipToasts } from "@gitbutler/ui";
	import { copyToClipboard } from "@gitbutler/ui/utils/clipboard";

	const loginService = inject(LOGIN_SERVICE);
	const BUILD_TYPE_PARAM = "bt";

	const searchParams = $derived(page.url.searchParams);
	const buildType = $derived(mapBuiltType(searchParams.get(BUILD_TYPE_PARAM)));

	async function copyAccessToken() {
		const response = await loginService.token();
		if (response.type === "success" && response.data) {
			copyToClipboard(response.data);
		} else {
			chipToasts.error("Token konnte nicht abgerufen werden");
		}
	}

	function mapBuiltType(buildType: string | null): "but" | "but-nightly" | null {
		switch (buildType) {
			case "release":
				return "but";
			case "nightly":
				return "but-nightly";
			default:
				return null;
		}
	}

	async function followDeeplink() {
		if (!buildType) {
			chipToasts.error("Unbekannter Build-Typ");
			return;
		}

		const response = await loginService.token();
		if (response.type !== "success" || !response.data) {
			chipToasts.error("Token konnte nicht abgerufen werden");
		} else {
			const accessToken = response.data;
			const deeplink = `${buildType}://login?access_token=${accessToken}&t=${Date.now()}`;
			window.location.href = deeplink;
		}
	}
</script>

<svelte:head>
	<title>GitButler | Angemeldet</title>
</svelte:head>

<FullscreenUtilityCard
	title="Erfolgreich angemeldet 🎯"
	backlink={{
		label: "Startseite",
		href: "/",
	}}
>
	<div class="loggedin__success-card-content">
		{#if buildType !== null}
			<p class="text-13">Klicke unten, um deinen Client zu öffnen und die Anmeldung abzuschließen.</p>
		{:else}
			<p class="text-13">Kopiere das Zugriffstoken und füge es in deinen Client ein.</p>
		{/if}
		<div class="flex gap-8 m-t-8">
			{#if buildType !== null}
				<AsyncButton style="gray" kind="outline" icon="open-in-ide" action={followDeeplink}
					>Client öffnen</AsyncButton
				>
			{:else}
				<AsyncButton style="gray" kind="outline" icon="copy" action={copyAccessToken}
					>Zugriffstoken kopieren</AsyncButton
				>
			{/if}
			<Button style="gray" kind="ghost" onclick={() => goto("/profile")} icon="user"
				>Profilseite</Button
			>
		</div>
	</div>
</FullscreenUtilityCard>

<style>
	.loggedin__success-card-content {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
</style>
