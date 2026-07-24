import { BranchIcon, CiIcon, ForgeIcon, OpenInBrowserIcon } from "./icons";
import {
	type McpUiToolResultNotification,
	useApp,
	useHostStyles,
} from "@modelcontextprotocol/ext-apps/react";
import { useEffect, useState } from "react";
import type { ForgeInfo } from "@gitbutler/but-sdk";

type ReviewState = "closed" | "draft" | "merged" | "open";

type ReviewPerson = {
	login: string;
	name?: string | null;
};

type ReviewCardData = {
	number: number;
	title: string;
	url: string;
	state: ReviewState;
	sourceBranch: string;
	targetBranch: string;
	author?: ReviewPerson | null;
	reviewers: ReviewPerson[];
	labels: string[];
	createdAt?: string | null;
	canMarkReady: boolean;
	ci: ReviewCi;
};

type ReviewCiStatus =
	| "actionRequired"
	| "cancelled"
	| "failure"
	| "inProgress"
	| "noChecks"
	| "success"
	| "unknown"
	| "unavailable"
	| "unsupported";

type ReviewCi = {
	status: ReviewCiStatus;
	total: number;
	passing: number;
	pending: number;
	failing: number;
	failingCheckNames: string[];
};

type ReviewView = {
	version: number;
	repository: {
		name: string;
		path: string;
	};
	forge: ForgeInfo;
	reviews: ReviewCardData[];
};

type ToolResult = McpUiToolResultNotification["params"];
const REVIEW_POLL_INTERVAL_MS = 30_000;

function textFromToolResult(result: ToolResult): string {
	return (
		result.content?.find((content) => content.type === "text")?.text ??
		"The review operation failed."
	);
}

function reviewViewFromToolResult(result: ToolResult): ReviewView | null {
	const value = result.structuredContent;
	if (
		typeof value !== "object" ||
		value === null ||
		!("repository" in value) ||
		!("forge" in value) ||
		!("reviews" in value)
	) {
		return null;
	}
	return value as ReviewView;
}

function reviewStateLabel(state: ReviewState): string {
	switch (state) {
		case "closed":
			return "Closed";
		case "draft":
			return "Draft";
		case "merged":
			return "Merged";
		case "open":
			return "Ready";
	}
}

function displayPerson(person: ReviewPerson): string {
	return person.name?.trim() || `@${person.login}`;
}

function createdLabel(value?: string | null): string | null {
	if (!value) return null;
	const date = new Date(value);
	if (Number.isNaN(date.getTime())) return null;
	return new Intl.DateTimeFormat(undefined, {
		dateStyle: "medium",
	}).format(date);
}

function ciLabel(ci: ReviewCi): string {
	switch (ci.status) {
		case "actionRequired":
			return "Action required";
		case "cancelled":
			return "CI cancelled";
		case "failure":
			return "CI failed";
		case "inProgress":
			return "CI running";
		case "noChecks":
			return "No checks";
		case "success":
			return "CI passed";
		case "unknown":
			return "CI unknown";
		case "unavailable":
			return "CI unavailable";
		case "unsupported":
			return "CI unsupported";
	}
}

function ciIcon(ci: ReviewCi): "cross" | "question" | "spinner" | "tick" | "warning" {
	switch (ci.status) {
		case "success":
			return "tick";
		case "cancelled":
		case "failure":
			return "cross";
		case "inProgress":
			return "spinner";
		case "actionRequired":
			return "warning";
		case "noChecks":
		case "unknown":
		case "unavailable":
		case "unsupported":
			return "question";
	}
}

function ciTitle(ci: ReviewCi): string {
	if (ci.status === "failure" && ci.failingCheckNames.length > 0) {
		return `Failed checks: ${ci.failingCheckNames.join(", ")}`;
	}
	if (ci.total > 0) {
		return `${ciLabel(ci)} · ${ci.passing} passed · ${ci.pending} pending · ${ci.failing} failed`;
	}
	return ciLabel(ci);
}

function CiStatus({ ci }: { ci: ReviewCi }) {
	if (ci.status === "unsupported") return null;

	return (
		<span className="ci-status" data-state={ci.status} title={ciTitle(ci)}>
			<CiIcon kind={ciIcon(ci)} />
			<span>{ciLabel(ci)}</span>
		</span>
	);
}

function shouldPollReview(review: ReviewCardData): boolean {
	const isOpen = review.state === "draft" || review.state === "open";
	return (
		isOpen &&
		review.ci.status !== "failure" &&
		review.ci.status !== "unavailable" &&
		review.ci.status !== "unsupported"
	);
}

function mergeReviewViews(current: ReviewView, update: ReviewView): ReviewView {
	const updatedByNumber = new Map(update.reviews.map((review) => [review.number, review]));
	return {
		...current,
		forge: update.forge,
		reviews: current.reviews.map((review) => updatedByNumber.get(review.number) ?? review),
	};
}

function ReviewCard({
	review,
	forge,
	canCallTools,
	pending,
	onMarkReady,
	onOpen,
}: {
	review: ReviewCardData;
	forge: ForgeInfo;
	canCallTools: boolean;
	pending: boolean;
	onMarkReady: () => void;
	onOpen: () => void;
}) {
	const created = createdLabel(review.createdAt);

	return (
		<article className="review-card">
			<header className="review-card-header">
				<div className="review-identity">
					<ForgeIcon className="forge-icon" name={forge.name} />
					<span>
						{forge.unit.abbr} {forge.unit.symbol}
						{review.number}
					</span>
				</div>
				<div className="review-card-controls">
					<CiStatus ci={review.ci} />
					<span className="review-status" data-state={review.state}>
						{reviewStateLabel(review.state)}
					</span>
					<button
						className="icon-button"
						disabled={pending}
						onClick={onOpen}
						type="button"
						title={`Open ${forge.unit.abbr} in browser`}
						aria-label={`Open ${forge.unit.abbr} in browser`}
					>
						<OpenInBrowserIcon />
					</button>
				</div>
			</header>

			<h2>{review.title}</h2>

			<div className="branch-flow" title={`${review.sourceBranch} → ${review.targetBranch}`}>
				<BranchIcon />
				<code>{review.sourceBranch}</code>
				<svg className="branch-arrow" viewBox="0 0 16 16" aria-hidden="true">
					<path d="M3 8h9M9 4l4 4-4 4" />
				</svg>
				<code>{review.targetBranch}</code>
			</div>

			<div className="review-facts">
				<span>{review.author ? displayPerson(review.author) : "Unknown author"}</span>
				<span aria-hidden="true">·</span>
				<span>
					{review.reviewers.length === 0
						? "No reviewers"
						: `${review.reviewers.length} ${
								review.reviewers.length === 1 ? "reviewer" : "reviewers"
							}`}
				</span>
				{created && (
					<>
						<span aria-hidden="true">·</span>
						<span>{created}</span>
					</>
				)}
			</div>

			{review.labels.length > 0 && (
				<div className="review-labels" aria-label="Labels">
					{review.labels.map((label) => (
						<span key={label}>{label}</span>
					))}
				</div>
			)}

			<footer className="review-actions">
				{review.canMarkReady && canCallTools && (
					<button
						className="review-button primary"
						disabled={pending}
						onClick={onMarkReady}
						type="button"
					>
						{pending && <span className="spinner" aria-hidden="true" />}
						{pending ? "Marking ready…" : "Ready for review"}
					</button>
				)}
			</footer>
		</article>
	);
}

export function ReviewApp() {
	const [view, setView] = useState<ReviewView | null>(null);
	const [resultError, setResultError] = useState<string | null>(null);
	const [actionError, setActionError] = useState<string | null>(null);
	const [pollingError, setPollingError] = useState<string | null>(null);
	const [pendingReview, setPendingReview] = useState<number | null>(null);
	const { app, isConnected, error } = useApp({
		appInfo: { name: "GitButler review", version: "1.0.0" },
		capabilities: {},
		onAppCreated: (createdApp) => {
			createdApp.addEventListener("toolresult", (result) => {
				if (result.isError) {
					setResultError(textFromToolResult(result));
					return;
				}
				const nextView = reviewViewFromToolResult(result);
				if (nextView === null) {
					setResultError("The review result did not contain structured data.");
					return;
				}
				setView(nextView);
				setResultError(null);
				setPollingError(null);
			});
		},
	});
	useHostStyles(app, app?.getHostContext());

	useEffect(() => {
		if (
			app === null ||
			view === null ||
			pollingError !== null ||
			app.getHostCapabilities()?.serverTools === undefined
		) {
			return;
		}

		const reviewNumbers = view.reviews.filter(shouldPollReview).map((review) => review.number);
		if (reviewNumbers.length === 0) return;

		let cancelled = false;

		async function refreshReviews() {
			try {
				const result = await app?.callServerTool({
					name: "gitbutler_refresh_reviews",
					arguments: {
						repository: view?.repository.path,
						reviewNumbers,
					},
				});
				if (cancelled || result === undefined) return;
				if (result.isError) throw new Error(textFromToolResult(result));
				const refreshed = reviewViewFromToolResult(result);
				if (refreshed === null) {
					throw new Error("The refreshed reviews were missing from the response.");
				}
				setView((current) => (current === null ? current : mergeReviewViews(current, refreshed)));
			} catch (pollCause) {
				if (cancelled) return;
				setPollingError(
					pollCause instanceof Error ? pollCause.message : "Could not refresh CI status.",
				);
			}
		}

		const timeout = window.setTimeout(() => void refreshReviews(), REVIEW_POLL_INTERVAL_MS);
		return () => {
			cancelled = true;
			window.clearTimeout(timeout);
		};
	}, [app, pollingError, view]);

	if (error !== null) {
		return (
			<div className="message-state error-state">
				Could not connect to the host: {error.message}
			</div>
		);
	}
	if (resultError !== null) {
		return <div className="message-state error-state">{resultError}</div>;
	}
	if (!isConnected || view === null || app === null) {
		return (
			<div className="message-state loading-state">
				<span className="spinner" aria-hidden="true" />
				Loading GitButler review…
			</div>
		);
	}

	const connectedApp = app;
	const currentView = view;
	const canCallTools = connectedApp.getHostCapabilities()?.serverTools !== undefined;

	async function markReady(review: ReviewCardData) {
		setPendingReview(review.number);
		setActionError(null);
		try {
			const result = await connectedApp.callServerTool({
				name: "gitbutler_mark_review_ready",
				arguments: {
					repository: currentView.repository.path,
					reviewNumber: review.number,
				},
			});
			if (result.isError) throw new Error(textFromToolResult(result));
			const updatedView = reviewViewFromToolResult(result);
			const updatedReview = updatedView?.reviews[0];
			if (!updatedReview) throw new Error("The updated review was missing from the response.");
			setView((current) =>
				current === null || updatedView === null ? current : mergeReviewViews(current, updatedView),
			);
		} catch (actionCause) {
			setActionError(
				actionCause instanceof Error ? actionCause.message : "Could not mark the review ready.",
			);
		} finally {
			setPendingReview(null);
		}
	}

	async function openReview(review: ReviewCardData) {
		setActionError(null);
		try {
			await connectedApp.openLink({ url: review.url });
		} catch (actionCause) {
			setActionError(
				actionCause instanceof Error ? actionCause.message : "Could not open the review.",
			);
		}
	}

	return (
		<main className="review-shell">
			<header className="review-context">
				<div>
					<span className="eyebrow">GitButler review</span>
					<h1>{view.repository.name}</h1>
				</div>
				<span className="review-count">
					{view.reviews.length} {view.reviews.length === 1 ? "review" : "reviews"}
				</span>
			</header>

			{actionError && (
				<div className="action-error" role="alert">
					{actionError}
				</div>
			)}
			{pollingError && (
				<div className="polling-error" role="status">
					CI status stopped updating: {pollingError}
				</div>
			)}

			<div className="review-list">
				{view.reviews.map((review) => (
					<ReviewCard
						key={review.number}
						review={review}
						forge={view.forge}
						canCallTools={canCallTools}
						pending={pendingReview === review.number}
						onMarkReady={() => void markReady(review)}
						onOpen={() => void openReview(review)}
					/>
				))}
			</div>
		</main>
	);
}
