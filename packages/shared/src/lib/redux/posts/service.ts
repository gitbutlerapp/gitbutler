import { InterestStore } from '$lib/redux/interest/intrestStore';
import {
	feedAppend,
	feedPrepend,
	upsertPost,
	upsertPostReplies,
	upsertPosts
} from '$lib/redux/posts/slice';
import {
	apiToPost,
	type ApiPost,
	type ApiPostWithReplies,
	type Post
} from '$lib/redux/posts/types';
import type { HttpClient } from '$lib/httpClient';
import type { AppDispatch } from '$lib/redux/store';

export class FeedService {
	private readonly feedInterests = new InterestStore<{ identifier: string }>(1 * 60 * 1000);
	private readonly postWithRepliesInterests = new InterestStore<{ postId: string }>(30 * 1000);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly dispatch: AppDispatch
	) {}

	/** Fetch and poll the latest entries in the feed */
	getFeedHeadInterest() {
		return this.feedInterests.createInterest({ identifier: 'all' }, () => {
			this.getFeedPage('all');
		});
	}

	async getFeedPage(_identifier: string, lastPostTimestamp?: string) {
		const query = lastPostTimestamp ? `?from_created_at=${lastPostTimestamp}` : '';
		const apiFeed = await this.httpClient.get<ApiPost[]>(`feed${query}`);
		this.dispatch(upsertPosts(apiFeed.map(apiToPost)));

		const actionArguments = { identifier: 'all', postIds: apiFeed.map((post) => post.uuid) };
		if (lastPostTimestamp) {
			this.dispatch(feedAppend(actionArguments));
		} else {
			this.dispatch(feedPrepend(actionArguments));
		}
	}

	async createPost(content: string): Promise<Post> {
		const apiPost = await this.httpClient.post<ApiPost>('feed/new', { body: { content } });
		const post = apiToPost(apiPost);
		this.dispatch(upsertPost(post));

		// TODO: Determine if this is needed / wanted / useful
		this.getFeedPage('all');

		return post;
	}

	getPostWithRepliesInterest(postId: string) {
		return this.postWithRepliesInterests.createInterest({ postId }, async () => {
			return;
			const apiPostWithReplies = await this.httpClient.get<ApiPostWithReplies>(
				`feed/post/${postId}`
			);
			const post = apiToPost(apiPostWithReplies);
			const posts = [post, ...apiPostWithReplies.replies.map(apiToPost)];
			this.dispatch(upsertPosts(posts));
			this.dispatch(
				upsertPostReplies({
					postId,
					replyIds: apiPostWithReplies.replies.map((reply) => reply.uuid)
				})
			);
		});
	}
}
