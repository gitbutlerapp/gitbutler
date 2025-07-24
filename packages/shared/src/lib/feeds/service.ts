import { InjectionToken } from '$lib/context';
import { feedAppend, feedPrepend } from '$lib/feeds/feedsSlice';
import { upsertPost, upsertPosts } from '$lib/feeds/postsSlice';
import { apiToPost, type ApiPost, type ApiPostWithReplies, type Post } from '$lib/feeds/types';
import { InterestStore } from '$lib/interest/interestStore';
import { POLLING_FAST, POLLING_REGULAR } from '$lib/polling';
import { guardReadableTrue } from '$lib/storeUtils';
import { apiToUser, type LoadableUser } from '$lib/users/types';
import { userTable } from '$lib/users/usersSlice';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

export const FEED_SERVICE = new InjectionToken<FeedService>('FeedService');

export class FeedService {
	private readonly feedInterests = new InterestStore<{ identifier: string }>(POLLING_REGULAR);
	private readonly postWithRepliesInterests = new InterestStore<{ postId: string }>(POLLING_FAST);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	/** Fetch and poll the latest entries in the feed */
	getFeedHeadInterest(identifier: string) {
		return this.feedInterests
			.findOrCreateSubscribable({ identifier }, () => {
				this.getFeedPage(identifier);
			})
			.createInterest();
	}

	/**
	 * Fetches either the first page of a feed, or a page of posts after a certain timestamp.
	 *
	 * Any posts returned get upserted into the posts slice.
	 * If the the fist page is queried, the ids will be added to the front of the feed.
	 * If a lastPostTimestamp is provided, the ids will be added to the end of the feed.
	 *
	 * If pages are queried out of order, the feed may end up with post ids that are not in order.
	 *
	 * TODO(CTO): This function is due some TLC, it has implicit behaviour and does not make me happy
	 */
	async getFeedPage(identifier: string, lastPostTimestamp?: string) {
		const query = lastPostTimestamp ? `?from_created_at=${lastPostTimestamp}` : '';
		const apiFeed = await this.httpClient.get<ApiPost[]>(`feed/project/${identifier}${query}`);
		this.appDispatch.dispatch(upsertPosts(apiFeed.map(apiToPost)));
		const users = apiFeed.map(
			(apiPost): LoadableUser => ({
				status: 'found',
				value: apiToUser(apiPost.user),
				id: apiPost.user.id
			})
		);
		this.appDispatch.dispatch(userTable.upsertMany(users));

		const actionArguments = { identifier, postIds: apiFeed.map((post) => post.uuid) };
		if (lastPostTimestamp) {
			this.appDispatch.dispatch(feedAppend(actionArguments));
		} else {
			this.appDispatch.dispatch(feedPrepend(actionArguments));
		}
	}

	async createPost(
		content: string,
		projectRepositoryId: string,
		identifier: string,
		replyTo?: string,
		picture?: File
	): Promise<Post> {
		await guardReadableTrue(this.httpClient.authenticationAvailable);

		const formData = new FormData();
		formData.append('content', content);
		formData.append('project_repository_id', projectRepositoryId);
		if (replyTo) {
			formData.append('reply_to', replyTo);
		}
		if (picture) {
			formData.append('picture', picture);
		}

		const apiPost = await this.httpClient.post<ApiPost>('feed/new', {
			body: formData,
			headers: { 'Content-Type': undefined }
		});

		const post = apiToPost(apiPost);
		this.appDispatch.dispatch(upsertPost(post));

		if (replyTo) {
			this.getPostWithReplies(replyTo);
		} else {
			this.getFeedPage(identifier);
		}

		return post;
	}

	getPostWithRepliesInterest(postId: string) {
		return this.postWithRepliesInterests
			.findOrCreateSubscribable({ postId }, async () => {
				this.getPostWithReplies(postId);
			})
			.createInterest();
	}

	private async getPostWithReplies(postId: string) {
		const apiPostWithReplies = await this.httpClient.get<ApiPostWithReplies>(`feed/post/${postId}`);
		const post = apiToPost(apiPostWithReplies);
		post.replyIds = apiPostWithReplies.replies.map((reply) => reply.uuid);

		const posts = [post, ...apiPostWithReplies.replies.map(apiToPost)];
		this.appDispatch.dispatch(upsertPosts(posts));
		const users = [apiPostWithReplies, ...apiPostWithReplies.replies].map(
			(apiPost): LoadableUser => ({
				status: 'found',
				id: apiPost.user.id,
				value: apiToUser(apiPost.user)
			})
		);
		this.appDispatch.dispatch(userTable.upsertMany(users));
	}
}
