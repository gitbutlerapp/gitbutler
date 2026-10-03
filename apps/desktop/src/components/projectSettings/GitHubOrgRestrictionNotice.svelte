<script lang="ts">
	import { t } from "$lib/i18n";

	import { InfoMessage, Link } from "@gitbutler/ui-svelte";

	import type { Code } from "@gitbutler/but-sdk";

	const { errorCode }: { errorCode: Code | undefined } = $props();
</script>

{#if errorCode === "GitHubOrgOAuthRestricted"}
	<InfoMessage style="warning" filled outlined={false}>
		{#snippet title()}
			{t("restricted-by-a-github-organization")}
		{/snippet}
		{#snippet content()}
			{t("an-organization-that-owns-this-repository-has-blocked-the-gi")}
			<Link
				href="https://docs.gitbutler.com/features/forge-integration/github-integration?utm_source=gitbutler-app&utm_medium=settings-banner&utm_campaign=org-oauth-restriction#connect-a-github-account"
				>docs</Link
			>.
		{/snippet}
	</InfoMessage>
{:else if errorCode === "GitHubOrgSamlRestricted"}
	<InfoMessage style="warning" filled outlined={false}>
		{#snippet title()}
			{t("github-organization-requires-saml-sso")}
		{/snippet}
		{#snippet content()}
			{t("this-repository-s-organization-requires-saml-sso-but-the-sel")}
		{/snippet}
	</InfoMessage>
{:else if errorCode === "GitHubTokenLifetimeRestricted"}
	<InfoMessage style="warning" filled outlined={false}>
		{#snippet title()}
			{t("github-organization-limits-token-lifetime")}
		{/snippet}
		{#snippet content()}
			{t("this-repository-s-organization-refuses-personal-access-token")}
		{/snippet}
	</InfoMessage>
{/if}
