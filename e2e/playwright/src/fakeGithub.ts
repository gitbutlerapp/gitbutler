import http, { type IncomingMessage, type ServerResponse } from "node:http";
import { spawn } from "node:child_process";
import path from "node:path";

export type FakeGitHubOptions = {
	headRepoPath?: string;
	forkRepoPath?: string;
	sourceBranch?: string;
	owner?: string;
	repo?: string;
	repoOwner?: string;
	reviewNumber?: number;
	title?: string;
	isFork?: boolean;
	listed?: boolean;
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
	setListed: (listed: boolean) => void;
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
	listed = true,
}: FakeGitHubOptions): Promise<FakeGitHubServer> {
	const reviewRepoPath = headRepoPath ?? forkRepoPath;
	if (!reviewRepoPath) {
		throw new Error("Fake GitHub review needs a headRepoPath or forkRepoPath");
	}

	const options = {
		headRepoPath: reviewRepoPath,
		sourceBranch,
		owner,
		repo,
		repoOwner,
		reviewNumber,
		title,
		isFork,
	};
	let currentReview = pullRequestPayload(options);
	let isListed = listed;

	const server = http.createServer((request, response) => {
		handleRequest(request, response, options, {
			get review() {
				return currentReview;
			},
			get listed() {
				return isListed;
			},
			setReview(review) {
				currentReview = review;
			},
		}).catch((error: unknown) => {
			response.writeHead(500, { "Content-Type": "application/json" });
			response.end(JSON.stringify({ message: String(error) }));
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
		setListed: (value) => {
			isListed = value;
		},
		close: async () =>
			await new Promise<void>((resolve, reject) => {
				server.close((error) => (error ? reject(error) : resolve()));
			}),
	};
}

async function handleRequest(
	request: IncomingMessage,
	response: ServerResponse,
	options: ResolvedFakeGitHubOptions,
	state: {
		readonly review: ReturnType<typeof pullRequestPayload>;
		readonly listed: boolean;
		setReview: (review: ReturnType<typeof pullRequestPayload>) => void;
	},
) {
	const url = new URL(request.url ?? "/", "http://127.0.0.1");
	const pullPath = `/api/v3/repos/${options.owner}/${options.repo}/pulls`;
	const review = state.review;

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
		return json(response, state.listed ? [review] : []);
	}

	if (request.method === "GET" && url.pathname === `${pullPath}/${options.reviewNumber}`) {
		return json(response, review);
	}

	if (request.method === "POST" && url.pathname === pullPath) {
		const body = JSON.parse(await readBody(request)) as {
			title?: string;
			head?: string;
			base?: string;
			body?: string | null;
			draft?: boolean;
		};
		const requestedHead = body.head ?? options.sourceBranch;
		const created = pullRequestPayload(
			{
				...options,
				title: body.title ?? options.title,
				sourceBranch: unqualifiedHeadRef(requestedHead),
			},
			requestedHead,
		);
		state.setReview(created);
		return json(response, created, 201);
	}

	const repositoryPath = `/${options.owner}/${options.repo}.git`;
	if (url.pathname === repositoryPath || url.pathname.startsWith(`${repositoryPath}/`)) {
		return serveGitRequest(request, response, url, repositoryPath, options.headRepoPath);
	}

	response.writeHead(404, { "Content-Type": "application/json" });
	response.end(
		JSON.stringify({ message: `No fake GitHub route for ${request.method} ${url.pathname}` }),
	);
}

function pullRequestPayload(
	{
		headRepoPath,
		sourceBranch,
		owner,
		repo,
		repoOwner,
		reviewNumber,
		title,
		isFork,
	}: ResolvedFakeGitHubOptions,
	headLabel = `${repoOwner}:${sourceBranch}`,
) {
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
			label: headLabel,
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

function unqualifiedHeadRef(head: string): string {
	return head.slice(head.lastIndexOf(":") + 1);
}

async function serveGitRequest(
	request: IncomingMessage,
	response: ServerResponse,
	url: URL,
	repositoryPath: string,
	localRepositoryPath: string,
) {
	const repositoryParent = path.dirname(localRepositoryPath);
	const repositoryName = path.basename(localRepositoryPath);
	const pathSuffix = url.pathname.slice(repositoryPath.length);
	const child = spawn("git", ["http-backend"], {
		env: {
			...process.env,
			GIT_HTTP_EXPORT_ALL: "1",
			GIT_PROJECT_ROOT: repositoryParent,
			PATH_INFO: `/${repositoryName}${pathSuffix}`,
			QUERY_STRING: url.search.slice(1),
			REQUEST_METHOD: request.method ?? "GET",
			CONTENT_TYPE: request.headers["content-type"] ?? "",
			CONTENT_LENGTH: request.headers["content-length"] ?? "",
			HTTP_GIT_PROTOCOL: request.headers["git-protocol"] ?? "",
			REMOTE_ADDR: request.socket.remoteAddress ?? "127.0.0.1",
		},
	});

	request.pipe(child.stdin);
	const stdout: Buffer[] = [];
	const stderr: Buffer[] = [];
	child.stdout.on("data", (chunk: Buffer) => stdout.push(chunk));
	child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk));

	const exitCode = await new Promise<number | null>((resolve, reject) => {
		child.on("error", reject);
		child.on("close", resolve);
	});
	if (exitCode !== 0) {
		throw new Error(`git http-backend failed: ${Buffer.concat(stderr).toString("utf8")}`);
	}

	const output = Buffer.concat(stdout);
	const headerEnd = output.indexOf("\r\n\r\n");
	if (headerEnd < 0) throw new Error("git http-backend returned malformed CGI output");
	const headers = output.subarray(0, headerEnd).toString("utf8").split("\r\n");
	let status = 200;
	for (const header of headers) {
		const separator = header.indexOf(":");
		if (separator < 0) continue;
		const name = header.slice(0, separator);
		const value = header.slice(separator + 1).trim();
		if (name.toLowerCase() === "status") status = Number.parseInt(value, 10);
		else response.setHeader(name, value);
	}
	response.writeHead(status);
	response.end(output.subarray(headerEnd + 4));
}

async function readBody(request: IncomingMessage): Promise<string> {
	const chunks: Buffer[] = [];
	for await (const chunk of request) {
		chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
	}
	return Buffer.concat(chunks).toString("utf8");
}

function json(response: ServerResponse, subject: unknown, status = 200) {
	response.writeHead(status, { "Content-Type": "application/json" });
	response.end(JSON.stringify(subject));
}
