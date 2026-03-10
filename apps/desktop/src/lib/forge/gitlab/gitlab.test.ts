import { GitLab } from "$lib/forge/gitlab/gitlab";
import { expect, test, describe, vi } from "vitest";
import type { GitLabClient } from "$lib/forge/gitlab/gitlabClient.svelte";
import type { AppDispatch, BackendApi, GitLabApi } from "$lib/state/clientState.svelte";

describe("GitLab", () => {
	// Mock GitLab API and client
	const gitLabApi = {
		endpoints: {},
		reducerPath: "gitlab",
		injectEndpoints: vi.fn(),
	} as unknown as GitLabApi;
	const gitLabClient = { onReset: () => {} } as unknown as GitLabClient;

	const MockBackendApi = vi.fn();
	MockBackendApi.prototype.injectEndpoints = vi.fn();
	const backendApi: BackendApi = new MockBackendApi();

	const baseBranch = "main";
	const baseRepo = {
		domain: "gitlab.example.com",
		name: "test-repo",
		owner: "test-owner",
	};

	test("uses https protocol by default when no protocol specified", () => {
		const gitlab = new GitLab({
			api: gitLabApi,
			backendApi,
			client: gitLabClient,
			repo: baseRepo,
			baseBranch,
			authenticated: true,
			dispatch: vi.fn() as unknown as AppDispatch,
			isLoading: false,
		});

		expect(gitlab.commitUrl("abc123")).toBe(
			"https://gitlab.example.com/test-owner/test-repo/-/commit/abc123",
		);
	});

	test("respects http protocol when specified", () => {
		const repo = {
			...baseRepo,
			protocol: "http",
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			backendApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true,
			dispatch: vi.fn() as unknown as AppDispatch,
			isLoading: false,
		});

		expect(gitlab.commitUrl("abc123")).toBe(
			"http://gitlab.example.com/test-owner/test-repo/-/commit/abc123",
		);
	});

	test("respects https protocol when explicitly specified", () => {
		const repo = {
			...baseRepo,
			protocol: "https",
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			backendApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true,
			dispatch: vi.fn() as unknown as AppDispatch,
			isLoading: false,
		});

		expect(gitlab.commitUrl("abc123")).toBe(
			"https://gitlab.example.com/test-owner/test-repo/-/commit/abc123",
		);
	});

	test("handles protocol with colon suffix", () => {
		const repo = {
			...baseRepo,
			protocol: "http:",
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			backendApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true,
			dispatch: vi.fn() as unknown as AppDispatch,
			isLoading: false,
		});

		expect(gitlab.commitUrl("abc123")).toBe(
			"http://gitlab.example.com/test-owner/test-repo/-/commit/abc123",
		);
	});

	test("branch urls use correct protocol", () => {
		const repo = {
			...baseRepo,
			protocol: "http",
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			backendApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true,
			dispatch: vi.fn() as unknown as AppDispatch,
			isLoading: false,
		});

		const branch = gitlab.branch("feature-branch");
		expect(branch?.url).toBe(
			"http://gitlab.example.com/test-owner/test-repo/-/compare/main...feature-branch",
		);
	});

	test("uses https protocol for ssh remote urls (browser compatibility)", () => {
		const repo = {
			...baseRepo,
			protocol: "ssh",
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			backendApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true,
			dispatch: vi.fn() as unknown as AppDispatch,
			isLoading: false,
		});

		expect(gitlab.commitUrl("abc123")).toBe(
			"https://gitlab.example.com/test-owner/test-repo/-/commit/abc123",
		);
	});

	test("branch urls use https protocol for ssh remote urls", () => {
		const repo = {
			...baseRepo,
			protocol: "ssh",
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			backendApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true,
			dispatch: vi.fn() as unknown as AppDispatch,
			isLoading: false,
		});

		const branch = gitlab.branch("feature-branch");
		expect(branch?.url).toBe(
			"https://gitlab.example.com/test-owner/test-repo/-/compare/main...feature-branch",
		);
	});

	test("handles ssh protocol with colon suffix", () => {
		const repo = {
			...baseRepo,
			protocol: "ssh:",
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			backendApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true,
			dispatch: vi.fn() as unknown as AppDispatch,
			isLoading: false,
		});

		expect(gitlab.commitUrl("abc123")).toBe(
			"https://gitlab.example.com/test-owner/test-repo/-/commit/abc123",
		);
	});

	test("uses https protocol for ssh remote urls on custom GitLab instance", () => {
		const repo = {
			domain: "gitlab.mycompany.com",
			name: "test-repo",
			owner: "test-owner",
			protocol: "ssh",
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			backendApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true,
			dispatch: vi.fn() as unknown as AppDispatch,
			isLoading: false,
		});

		expect(gitlab.commitUrl("abc123")).toBe(
			"https://gitlab.mycompany.com/test-owner/test-repo/-/commit/abc123",
		);
	});
});
