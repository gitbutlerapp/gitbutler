import http, { type IncomingMessage, type ServerResponse } from "node:http";

type FakeGitHubOptions = {
	headRepoPath?: string;
	forkRepoPath?: string;
	sourceBranch?: string;
	owner?: string;
	repo?: string;
	repoOwner?: string;
	reviewNumber?: number;
	title?: string;
	isFork?: boolean;
};

type ResolvedFakeGitHubOptions = {
	headRepoPath: string;
	sourceBranch: string;
	owner: string;
	repo: string;
	repoOwner: string;
	reviewNumber: number;
	title: string;
	isFork: boolean;
};

export type FakeGitHubServer = {
	apiBaseUrl: string;
	repositoryUrl: string;
	close: () => Promise<void>;
};

export async function startFakeGitHubServer({
	headRepoPath,
	forkRepoPath,
	sourceBranch = "fork-feature",
	owner = "acme",
	repo = "widgets",
	repoOwner = "Contributor User",
	reviewNumber = 42,
	title = "Fork PR",
	isFork = true,
}: FakeGitHubOptions): Promise<FakeGitHubServer> {
	const reviewRepoPath = headRepoPath ?? forkRepoPath;
	if (!reviewRepoPath) {
		throw new Error("Fake GitHub review needs a headRepoPath or forkRepoPath");
	}

	const server = http.createServer((request, response) => {
		handleRequest(request, response, {
			headRepoPath: reviewRepoPath,
			sourceBranch,
			owner,
			repo,
			repoOwner,
			reviewNumber,
			title,
			isFork,
		});
	});

	await new Promise<void>((resolve) => {
		server.listen(0, "127.0.0.1", resolve);
	});

	const address = server.address();
	if (!address || typeof address === "string") {
		throw new Error("Fake GitHub server did not bind to a TCP port");
	}

	const root = `http://127.0.0.1:${address.port}`;
	return {
		apiBaseUrl: `${root}/api/v3`,
		repositoryUrl: `${root}/${owner}/${repo}.git`,
		close: async () =>
			await new Promise<void>((resolve, reject) => {
				server.close((error) => (error ? reject(error) : resolve()));
			}),
	};
}

function handleRequest(
	request: IncomingMessage,
	response: ServerResponse,
	options: ResolvedFakeGitHubOptions,
) {
	const url = new URL(request.url ?? "/", "http://127.0.0.1");
	const pullPath = `/api/v3/repos/${options.owner}/${options.repo}/pulls`;
	const review = pullRequestPayload(options);

	if (request.method === "GET" && url.pathname === "/api/v3/user") {
		return json(response, {
			id: 1,
			login: "e2e-user",
			name: "E2E User",
			email: null,
			avatar_url: null,
			type: "User",
		});
	}

	if (request.method === "GET" && url.pathname === pullPath) {
		return json(response, [review]);
	}

	if (request.method === "GET" && url.pathname === `${pullPath}/${options.reviewNumber}`) {
		return json(response, review);
	}

	response.writeHead(404, { "Content-Type": "application/json" });
	response.end(
		JSON.stringify({ message: `No fake GitHub route for ${request.method} ${url.pathname}` }),
	);
}

function pullRequestPayload({
	headRepoPath,
	sourceBranch,
	owner,
	repo,
	repoOwner,
	reviewNumber,
	title,
	isFork,
}: ResolvedFakeGitHubOptions) {
	return {
		html_url: `http://127.0.0.1/${owner}/${repo}/pull/${reviewNumber}`,
		number: reviewNumber,
		title,
		body: null,
		user: null,
		labels: [],
		draft: false,
		merge_commit_sha: null,
		head: {
			ref: sourceBranch,
			sha: "0000000000000000000000000000000000000000",
			repo: {
				ssh_url: headRepoPath,
				clone_url: headRepoPath,
				owner: {
					id: 2,
					login: repoOwner,
					name: repoOwner,
					email: null,
					avatar_url: null,
					type: "User",
				},
				fork: isFork,
			},
		},
		base: {
			ref: "master",
			sha: "0000000000000000000000000000000000000000",
			repo: {
				ssh_url: `git@example.com:${owner}/${repo}.git`,
				clone_url: `https://example.com/${owner}/${repo}.git`,
				owner: {
					id: 3,
					login: owner,
					name: owner,
					email: null,
					avatar_url: null,
					type: "Organization",
				},
				fork: false,
			},
		},
		created_at: "2026-06-01T00:00:00Z",
		updated_at: "2026-06-01T00:00:00Z",
		merged_at: null,
		closed_at: null,
		requested_reviewers: [],
	};
}

function json(response: ServerResponse, subject: unknown) {
	response.writeHead(200, { "Content-Type": "application/json" });
	response.end(JSON.stringify(subject));
}
