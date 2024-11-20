import { InterestStore } from '$lib/redux/interest/intrestStore';
import { upsertFeed, upsertPost, upsertPostReplies, upsertPosts } from '$lib/redux/posts/slice';
import {
	apiToPost,
	type ApiPost,
	type ApiPostWithReplies,
	type Post
} from '$lib/redux/posts/types';
import type { HttpClient } from '$lib/httpClient';
import type { AppDispatch } from '$lib/redux/store';

export class FeedService {
	private readonly feedInterests = new InterestStore<undefined>(5 * 60 * 1000);
	private readonly postWithRepliesIntrestes = new InterestStore<{ postId: string }>(5 * 60 * 1000);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly dispatch: AppDispatch
	) {}

	/** Used to start querying and polling the feed */
	getFeedInterest() {
		return this.feedInterests.createInterest(undefined, async () => {
			const apiFeed = await this.httpClient.get<ApiPost[]>('feed');
			this.dispatch(upsertPosts(apiFeed.map(apiToPost)));
			this.dispatch(upsertFeed({ identifier: 'all', postIds: apiFeed.map((post) => post.uuid) }));
		});
	}

	async createPost(content: string): Promise<Post> {
		const apiPost = await this.httpClient.post<ApiPost>('feed/new', { body: { content } });
		const post = apiToPost(apiPost);
		this.dispatch(upsertPost(post));

		// TODO: Determine if this is needed / wanted / useful
		const apiFeed = await this.httpClient.get<ApiPost[]>('feed');
		this.dispatch(upsertPosts(apiFeed.map(apiToPost)));
		this.dispatch(upsertFeed({ identifier: 'all', postIds: apiFeed.map((post) => post.uuid) }));

		return post;
	}

	getPostWithRepliesInterest(postId: string) {
		return this.postWithRepliesIntrestes.createInterest({ postId }, async () => {
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
