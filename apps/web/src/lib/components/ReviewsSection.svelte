<script lang="ts">
	import { getRelativeTime } from '$lib/utils/dateUtils';
	import Button from '@gitbutler/ui/Button.svelte';
	import CommitStatusBadge, { type CommitStatusType } from '@gitbutler/ui/CommitStatusBadge.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import type { Branch } from '@gitbutler/shared/branches/types';
	import { goto } from '$app/navigation';

	interface Contributor {
		user?: {
			name?: string;
			avatarUrl?: string;
		};
	}

	interface BaseReview {
		uuid: string;
		projectFullSlug: string;
		stackSize?: number;
		updatedAt: string;
		contributors: Contributor[];
		reviewStatus: string;
		version?: number;
	}

	// Our own mock review type
	interface CustomReview extends BaseReview {
		title: string;
		status?: string;
		reviewUrl?: string;
	}

	// Union type to accept both Branch and CustomReview
	type Review = Branch | CustomReview;

	type LoadingStatus = 'loading' | 'found' | 'error' | 'not-found';

	interface Props {
		reviews: Review[];
		status?: LoadingStatus;
		sectionTitle?: string;
		allReviewsUrl?: string; // Optional URL for "All Reviews" link
		reviewsCount?: number; // Optional count of total reviews
	}

	let {
		reviews,
		status = 'found',
		sectionTitle = 'Recent Reviews',
		allReviewsUrl = undefined,
		reviewsCount = 0
	}: Props = $props();

	// Helper function to make Branch type compatible with our component's expectations
	function getTitle(review: Review): string {
		return 'title' in review && typeof review.title === 'string'
			? review.title
			: 'title' in review && review.title !== undefined
				? String(review.title)
				: 'Untitled Review';
	}

	function getReviewUrl(review: Review): string {
		return 'reviewUrl' in review && review.reviewUrl
			? review.reviewUrl
			: `/${review.projectFullSlug}/review/${review.uuid}`;
	}

	// Helper function to convert contributors to AvatarGroup format
	function getContributorAvatars(contributors: Contributor[]) {
		return contributors.map((contributor) => ({
			srcUrl: contributor.user?.avatarUrl || '/images/default-avatar.png',
			name: contributor.user?.name || 'User'
		}));
	}
</script>

<div class="section-card reviews-table-section">
	<div class="section-header">
		<h2 class="section-title">{sectionTitle}</h2>
		{#if allReviewsUrl && reviewsCount > 0}
			<Button onclick={() => goto(allReviewsUrl)} style="pop">All Reviews</Button>
		{/if}
	</div>

	{#if reviews.length > 0}
		<table class="reviews-table">
			<thead>
				<tr>
					<th>Status</th>
					<th>Project</th>
					<th>Name</th>
					<th>Commits</th>
					<th>Update</th>
					<th>Authors</th>
					<th title="Commit version">Ver.</th>
				</tr>
			</thead>
			<tbody>
				{#each reviews as review, i}
					<tr
						class="review-row {i === 0 ? 'first-row' : ''} {i === reviews.length - 1
							? 'last-row'
							: ''}"
					>
						<td>
							<CommitStatusBadge status={review.reviewStatus as CommitStatusType} />
						</td>
						<td>
							<a href={`/${review.projectFullSlug}`}>{review.projectFullSlug}</a>
						</td>
						<td>
							<a href={getReviewUrl(review)} class="review-title-link">
								{getTitle(review)}
							</a>
						</td>
						<td>{review.stackSize || '-'}</td>
						<td>{getRelativeTime(review.updatedAt)}</td>
						<td>
							<AvatarGroup avatars={getContributorAvatars(review.contributors)} />
						</td>
						<td>v{review.version || '-'}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{:else if status === 'loading'}
		<div class="loading-state">
			<div class="loading-spinner"></div>
			<p>Loading reviews...</p>
		</div>
	{:else}
		<div class="empty-state">
			<p>No recent reviews</p>
			<p class="empty-state-subtitle">Reviews will appear here once they are created.</p>
		</div>
	{/if}
</div>

<style>
	.section-card {
		background-color: white;
		border-radius: 8px;
		overflow: hidden;
		border: 1px solid color(srgb 0.831373 0.815686 0.807843);
	}

	.section-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		border-bottom: 1px solid color(srgb 0.831373 0.815686 0.807843);
		background-color: #f3f3f2;
		padding-right: 5px;
	}

	.section-title {
		font-size: 0.8em;
		margin: 0;
		padding: 12px 15px;
		color: color(srgb 0.52549 0.494118 0.47451);
	}

	/* Reviews Table */
	.empty-state {
		padding: 3rem 1rem;
		text-align: center;
		color: #718096;
	}

	.empty-state p {
		margin: 0 0 0.5rem 0;
		font-size: 1.1rem;
	}

	.empty-state-subtitle {
		font-size: 0.9rem !important;
		opacity: 0.8;
	}

	.loading-state {
		padding: 3rem 1rem;
		text-align: center;
		color: #718096;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
	}

	.loading-state p {
		margin: 1rem 0 0 0;
		font-size: 1.1rem;
	}

	.loading-spinner {
		width: 40px;
		height: 40px;
		border: 3px solid rgba(0, 0, 0, 0.1);
		border-radius: 50%;
		border-top-color: #2563eb;
		animation: spin 1s ease-in-out infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.reviews-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 13px;
		color: var(--clr-text-2);
	}

	.reviews-table thead {
		background-color: #eee;
		border-bottom: 1px solid color(srgb 0.831373 0.815686 0.807843);
	}

	.reviews-table th {
		text-align: left;
		padding: 10px 15px;
		font-weight: 500;
		color: color(srgb 0.52549 0.494118 0.47451);
		font-size: 0.8em;
	}

	.reviews-table td {
		padding: 18px 15px;
		border-bottom: 1px solid #e2e8f0;
		vertical-align: middle;
	}

	.review-row {
		background-color: white;
		transition: background-color 0.2s;
	}

	.review-row:hover {
		background-color: #f7fafc;
	}

	.first-row td:first-child {
		border-top-left-radius: 6px;
	}

	.first-row td:last-child {
		border-top-right-radius: 6px;
	}

	.last-row td:first-child {
		border-bottom-left-radius: 6px;
	}

	.last-row td:last-child {
		border-bottom-right-radius: 6px;
	}

	.review-title-link {
		color: #000;
		text-decoration: none;
		font-weight: 800;
		display: block;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: 300px;
	}

	.review-title-link:hover {
		text-decoration: underline;
	}
</style>
