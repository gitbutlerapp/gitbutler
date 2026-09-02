import GitHubOrgRestrictionNotice from "$components/projectSettings/GitHubOrgRestrictionNotice.svelte";
import { cleanup, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, test } from "vitest";

import type { Code } from "@gitbutler/but-sdk";

afterEach(cleanup);

function renderNotice(errorCode: Code | undefined) {
	return render(GitHubOrgRestrictionNotice, { props: { errorCode } });
}

describe("GitHubOrgRestrictionNotice", () => {
	test("preserves the OAuth restriction notice and docs link", () => {
		renderNotice("GitHubOrgOAuthRestricted");

		expect(screen.getByText("Restricted by a GitHub organization")).toBeTruthy();
		expect(screen.getByRole("link", { name: /docs/ }).getAttribute("href")).toContain(
			"utm_campaign=org-oauth-restriction",
		);
		expect(screen.queryByText("GitHub organization requires SAML SSO")).toBeNull();
	});

	test("renders actionable SAML guidance for the dedicated code", () => {
		renderNotice("GitHubOrgSamlRestricted");

		expect(screen.getByText("GitHub organization requires SAML SSO")).toBeTruthy();
		expect(
			screen.getByText(/authorize the GitButler OAuth app or your personal access token/),
		).toBeTruthy();
		expect(screen.queryByText("Restricted by a GitHub organization")).toBeNull();
	});

	test("renders no organization notice for unrelated errors", () => {
		const { container } = renderNotice("Unknown");

		expect(container.textContent).toBe("");
	});
});
