import { feedAppend, feedPrepend } from '$lib/feeds/feedsSlice';
import { upsertPost, upsertPosts } from '$lib/feeds/postsSlice';
import { apiToPost, type ApiPost, type ApiPostWithReplies, type Post } from '$lib/feeds/types';
import { InterestStore } from '$lib/interest/intrestStore';
import { POLLING_FAST, POLLING_REGULAR } from '$lib/polling';
import type { HttpClient } from '$lib/httpClient';
import type { AppDispatch } from '$lib/redux/store';

export class FeedService {
	private readonly feedInterests = new InterestStore<{ identifier: string }>(POLLING_REGULAR);
	private readonly postWithRepliesInterests = new InterestStore<{ postId: string }>(POLLING_FAST);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	/** Fetch and poll the latest entries in the feed */
	getFeedHeadInterest() {
		return this.feedInterests
			.findOrCreateSubscribable({ identifier: 'all' }, () => {
				this.getFeedPage('all');
			})
			.createInterest();
	}

	async getFeedPage(_identifier: string, lastPostTimestamp?: string) {
		const query = lastPostTimestamp ? `?from_created_at=${lastPostTimestamp}` : '';
		const apiFeed = await this.httpClient.get<ApiPost[]>(`feed${query}`);
		this.appDispatch.dispatch(upsertPosts(apiFeed.map(apiToPost)));

		const actionArguments = { identifier: 'all', postIds: apiFeed.map((post) => post.uuid) };
		if (lastPostTimestamp) {
			this.appDispatch.dispatch(feedAppend(actionArguments));
		} else {
			this.appDispatch.dispatch(feedPrepend(actionArguments));
		}
	}

	async createPost(content: string): Promise<Post> {
		const apiPost = await this.httpClient.post<ApiPost>('feed/new', { body: { content } });
		const post = apiToPost(apiPost);
		this.appDispatch.dispatch(upsertPost(post));

		// TODO: Determine if this is needed / wanted / useful
		this.getFeedPage('all');

		return post;
	}

	getPostWithRepliesInterest(postId: string) {
		return this.postWithRepliesInterests
			.findOrCreateSubscribable({ postId }, async () => {
				const apiPostWithReplies = await this.httpClient.get<ApiPostWithReplies>(
					`feed/post/${postId}`
				);
				const post = apiToPost(apiPostWithReplies);
				post.replyIds = apiPostWithReplies.replies.map((reply) => reply.uuid);

				const posts = [post, ...apiPostWithReplies.replies.map(apiToPost)];
				this.appDispatch.dispatch(upsertPosts(posts));
			})
			.createInterest();
	}
}
